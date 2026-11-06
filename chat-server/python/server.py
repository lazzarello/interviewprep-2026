import socket

import zmq
import asyncio
import zmq.asyncio



def main():
    ctx = zmq.Context()
    pub_sock = ctx.socket(zmq.PUB)
    pub_sock.bind("tcp://*:3000")

    rep_sock = ctx.socket(zmq.REP)
    rep_sock.bind("tcp://*:3001")
    # sub_sock.setsockopt_string(zmq.SUBSCRIBE, "chat")

    while True:
        sender, msg = rep_sock.recv_multipart()
        rep_sock.send_multipart([b"ack"])
        
        print(f"{sender} {msg}")
        pub_sock.send_multipart([b"chat", sender, msg])
        
    

if __name__ == '__main__':
    main()
