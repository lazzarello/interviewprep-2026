fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = zmq::Context::new();
    let req_socket = context.socket(zmq::REQ)?;
    req_socket.connect("tcp://localhost:3001")?;

    let sub_socket = context.socket(zmq::SUB)?;
    sub_socket.connect("tcp://localhost:3000")?;
    sub_socket.set_subscribe(b"chat")?;

    let _receiver = std::thread::spawn(move || -> Result<(), zmq::Error> {
        loop {
            let mesage = sub_socket.recv_multipart(0)?;
            for frame in &mesage {
                println!("{}", String::from_utf8_lossy(frame));
            }
        }
    });
    loop {
        let mut input = String::new();
        let input_bytes = std::io::stdin().read_line(&mut input)?;
        if input_bytes == 0 {
            break;
        }
        let message = input.trim_end_matches(['\r', '\n']).as_bytes().to_vec();
        let frames = vec![b"Alice".to_vec(), message];
        req_socket.send_multipart(frames, 0)?;
        let result = req_socket.recv_multipart(0)?;
        for frame in &result {
            println!("{}", String::from_utf8_lossy(frame));
        }
    }
    // Send a greeting on connect
    // let message = vec![b"Alice".to_vec(), b"hello from alice".to_vec()];
    // req_socket.send_multipart(input, 0)?;
    // Recieve the ack
    // let result = req_socket.recv_multipart(0)?;
    // for frame in &result {
    //     println!("{}", String::from_utf8_lossy(frame));
    // }
    _receiver;
    Ok(())
}
