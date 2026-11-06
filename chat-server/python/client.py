import asyncio
import socket
import sys

import zmq
import zmq.asyncio
import time


async def main(name):
    ctx = zmq.asyncio.Context()
    req_sock = ctx.socket(zmq.REQ)
    req_sock.connect("tcp://localhost:3001")

    sub_sock = ctx.socket(zmq.SUB)
    sub_sock.connect("tcp://localhost:3000")
    sub_sock.setsockopt_string(zmq.SUBSCRIBE, "chat")

    await req_sock.send_multipart([name.encode(), f"hello from {name}".encode()])
    await req_sock.recv_multipart()

    task_1 = asyncio.create_task(get_user_input(req_sock, name))
    task_2 = asyncio.create_task(recv(sub_sock, name))

    await asyncio.gather(task_1, task_2)


async def get_user_input(req_sock, name):
    """Reads user input without blocking the main event loop."""
    # asyncio.to_thread runs the blocking input() function in a separate worker thread
    # user_text = await asyncio.to_thread(input, "\n>")
    while True:
        user_input = await ainput("\n>")
        await req_sock.send_multipart([name.encode(), f"{user_input}".encode()])
        await req_sock.recv_multipart()


async def ainput(string: str) -> str:
    await asyncio.to_thread(sys.stdout.write, f"{string} ")
    return await asyncio.to_thread(sys.stdin.readline)


async def recv(sub_sock, name):
    while True:
        # await asyncio.sleep(0.2)
        topic, sender, msg = await sub_sock.recv_multipart()
        # ignore my own messages
        if sender == name.encode():
            continue
        print(f"{sender} {msg}")

        # pub_sock.send_multipart(topipc, sender, msg)


if __name__ == "__main__":
    asyncio.run(main(sys.argv[1]))
