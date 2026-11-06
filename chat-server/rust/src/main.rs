fn main() -> Result<(), zmq::Error> {
    let context = zmq::Context::new();
    let pub_socket = context.socket(zmq::PUB)?;
    pub_socket.bind("tcp://*:3000")?;

    let rep_socket = context.socket(zmq::REP)?;
    rep_socket.bind("tcp://*:3001")?;
    loop {
        let mut result = rep_socket.recv_multipart(0)?;
        if result.len() != 2 {
            rep_socket.send("error: expected sender and message", 0)?;
            continue;
        }
        for frame in &result {
            println!("{}", String::from_utf8_lossy(frame));
        }
        // A single-frame send is compatible with Python’s recv_multipart().
        rep_socket.send("ack", 0)?;
        // Here, .to_vec() turns the byte string into an owned Vec<u8>, matching the other frames. No
        // custom traits or iterator construction needed—the vector already supports iteration.
        result.insert(0, b"chat".to_vec());
        pub_socket.send_multipart(result, 0)?;
    }
}
