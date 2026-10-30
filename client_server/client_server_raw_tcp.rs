//! Thread-per-client TCP server with newline-delimited UTF-8 messages.
//! shortcut: no TLS, authentication, or connection limit, add these before exposing beyond localhost.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 9999;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

fn read_frame(reader: &mut impl BufRead, max_bytes: usize) -> io::Result<Option<String>> {
    let mut line = String::new();
    let bytes = reader.take((max_bytes + 1) as u64).read_line(&mut line)?;
    if bytes > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Frame is too large",
        ));
    }
    if bytes == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_owned()))
}

fn handle_client(mut conn: TcpStream, addr: SocketAddr) -> io::Result<()> {
    conn.set_read_timeout(Some(IO_TIMEOUT))?;
    conn.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(conn.try_clone()?);
    while let Some(message) = read_frame(&mut reader, MAX_FRAME_BYTES)? {
        if message.is_empty() || message == "QUIT" {
            break;
        }
        println!("[Server] Received from {addr}: '{message}'");
        writeln!(conn, "ACK: {message}")?;
        conn.flush()?;
    }
    Ok(())
}

fn run_server(listener: TcpListener, stop: Arc<AtomicBool>) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    println!("[Server] Listening on {}...", listener.local_addr()?);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((conn, addr)) => {
                conn.set_nonblocking(false)?;
                thread::Builder::new().spawn(move || {
                    if let Err(error) = handle_client(conn, addr) {
                        eprintln!("[Server] Client {addr}: {error}");
                    }
                })?;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn send_request(address: SocketAddr, message: &str) -> io::Result<String> {
    if message.contains(['\r', '\n']) || message.len() >= MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Request must fit one newline-delimited frame of at most 64 KiB",
        ));
    }
    let mut sock = TcpStream::connect_timeout(&address, IO_TIMEOUT)?;
    sock.set_read_timeout(Some(IO_TIMEOUT))?;
    sock.set_write_timeout(Some(IO_TIMEOUT))?;
    writeln!(sock, "{message}")?;
    sock.flush()?;
    Ok(read_frame(&mut BufReader::new(sock), MAX_FRAME_BYTES + "ACK: ".len())?.unwrap_or_default())
}

fn main() -> io::Result<()> {
    // Bind before spawning so clients do not race server startup.
    let listener = TcpListener::bind((HOST, PORT))?;
    let address = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = Arc::clone(&stop);
    let server = thread::spawn(move || run_server(listener, server_stop));
    let requests = (|| -> io::Result<()> {
        for message in ["ping", "hello world", "interview test"] {
            let response = send_request(address, message)?;
            println!("[Client] Sent '{message}', received '{response}'");
        }
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    server
        .join()
        .map_err(|_| io::Error::other("Server thread panicked"))??;
    requests?;
    println!("[Main] Server stopped.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::Shutdown;

    fn connection_pair() -> (TcpStream, thread::JoinHandle<io::Result<()>>) {
        let listener = TcpListener::bind((HOST, 0)).unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
        client.set_write_timeout(Some(IO_TIMEOUT)).unwrap();
        let (conn, addr) = listener.accept().unwrap();
        (client, thread::spawn(move || handle_client(conn, addr)))
    }

    #[test]
    fn framing_handles_small_reads_unicode_crlf_and_unterminated_eof() {
        let data = Cursor::new("  ping \r\nこんにちは\nlast frame".as_bytes());
        let mut reader = BufReader::with_capacity(1, data);
        assert_eq!(
            read_frame(&mut reader, MAX_FRAME_BYTES).unwrap().as_deref(),
            Some("ping")
        );
        assert_eq!(
            read_frame(&mut reader, MAX_FRAME_BYTES).unwrap().as_deref(),
            Some("こんにちは")
        );
        assert_eq!(
            read_frame(&mut reader, MAX_FRAME_BYTES).unwrap().as_deref(),
            Some("last frame")
        );
        assert_eq!(read_frame(&mut reader, MAX_FRAME_BYTES).unwrap(), None);
        assert_eq!(
            read_frame(&mut Cursor::new(b" \t\n"), MAX_FRAME_BYTES).unwrap(),
            Some(String::new())
        );
    }

    #[test]
    fn framing_rejects_invalid_utf8_and_limits_frame_size() {
        let mut exact = vec![b'x'; MAX_FRAME_BYTES - 1];
        exact.push(b'\n');
        assert_eq!(
            read_frame(&mut Cursor::new(exact), MAX_FRAME_BYTES)
                .unwrap()
                .unwrap()
                .len(),
            MAX_FRAME_BYTES - 1
        );
        let oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
        assert_eq!(
            read_frame(&mut Cursor::new(oversized), MAX_FRAME_BYTES)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read_frame(&mut Cursor::new([0xff, b'\n']), MAX_FRAME_BYTES)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn tcp_connection_handles_fragmented_and_coalesced_requests() {
        let (mut client, handler) = connection_pair();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        client.write_all(b"pi").unwrap();
        client.write_all("ng\nhello 世界\n".as_bytes()).unwrap();
        assert_eq!(
            read_frame(&mut reader, MAX_FRAME_BYTES).unwrap().as_deref(),
            Some("ACK: ping")
        );
        assert_eq!(
            read_frame(&mut reader, MAX_FRAME_BYTES).unwrap().as_deref(),
            Some("ACK: hello 世界")
        );
        client.write_all(b"QUIT\n").unwrap();
        assert_eq!(read_frame(&mut reader, MAX_FRAME_BYTES).unwrap(), None);
        handler.join().unwrap().unwrap();
    }

    #[test]
    fn blank_and_quit_close_without_an_ack_and_eof_finishes_a_partial_frame() {
        for message in ["QUIT\n", " \t\r\n"] {
            let (mut client, handler) = connection_pair();
            client.write_all(message.as_bytes()).unwrap();
            assert_eq!(
                read_frame(&mut BufReader::new(client), MAX_FRAME_BYTES).unwrap(),
                None
            );
            handler.join().unwrap().unwrap();
        }
        let (mut client, handler) = connection_pair();
        client.write_all(b"last frame").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        assert_eq!(
            read_frame(&mut BufReader::new(client), MAX_FRAME_BYTES)
                .unwrap()
                .as_deref(),
            Some("ACK: last frame")
        );
        handler.join().unwrap().unwrap();
    }

    #[test]
    fn client_rejects_multiline_and_oversized_requests_before_connecting() {
        let unused = SocketAddr::from(([127, 0, 0, 1], 0));
        for message in [
            "first\nsecond",
            "first\rsecond",
            &"x".repeat(MAX_FRAME_BYTES),
        ] {
            assert_eq!(
                send_request(unused, message).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[test]
    fn server_accepts_concurrent_clients_and_stops_listening() {
        let listener = TcpListener::bind((HOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let server = thread::spawn(move || run_server(listener, server_stop));
        let clients: Vec<_> = (0..6)
            .map(|id| thread::spawn(move || send_request(address, &format!("  client {id}  "))))
            .collect();
        // Stop even if a client failed, before asserting its result.
        let responses: Vec<_> = clients.into_iter().map(|client| client.join()).collect();
        let largest_reply = send_request(address, &"x".repeat(MAX_FRAME_BYTES - 1));
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap().unwrap();
        for (id, response) in responses.into_iter().enumerate() {
            assert_eq!(response.unwrap().unwrap(), format!("ACK: client {id}"));
        }
        assert_eq!(
            largest_reply.unwrap().len(),
            MAX_FRAME_BYTES - 1 + "ACK: ".len()
        );
        assert!(TcpStream::connect_timeout(&address, IO_TIMEOUT).is_err());
    }
}
