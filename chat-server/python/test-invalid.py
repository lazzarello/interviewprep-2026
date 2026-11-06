import asyncio
import socket
import sys

import zmq
import zmq.asyncio
import time


async def main():
    ctx = zmq.asyncio.Context()
    req_sock = ctx.socket(zmq.REQ)
    req_sock.connect("tcp://localhost:3001")

    await req_sock.send_multipart(["Alice".encode()])
    response = await req_sock.recv_multipart()
    print(response)
    await req_sock.send_multipart(["Alice".encode(), "Still working".encode()])
    response = await req_sock.recv_multipart()
    print(response)


if __name__ == "__main__":
    asyncio.run(main())
