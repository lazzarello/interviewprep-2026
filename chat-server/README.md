# Lab report: a ZeroMQ chat server and client in Rust

## Objective

Port the Python chat example to Rust, keeping the same wire protocol so Python
and Rust clients can use the same server. Build it incrementally, with the
learner writing the implementation and testing each milestone.

The result is a working MVP: a synchronous Rust server and a Rust client that
uses two OS threads for keyboard input and incoming broadcasts. No async runtime
was needed.

## Files

| File | Role |
| --- | --- |
| [`python/server.py`](python/server.py) | Original synchronous ZeroMQ server |
| [`python/client.py`](python/client.py) | Original asyncio client; accepts a name argument |
| [`python/test-invalid.py`](python/test-invalid.py) | Manual malformed-request and recovery probe |
| [`rust/src/main.rs`](rust/src/main.rs) | Rust server, executable named `rust` |
| [`rust/src/bin/client.rs`](rust/src/bin/client.rs) | Rust client, executable named `client` |
| [`rust/Cargo.toml`](rust/Cargo.toml) | Rust package using `zmq = "0.10.0"` |

## Architecture and protocol

```text
Client REQ ---- [sender, message] ----> Server REP :3001
           <-------- [ack] -----------

Client SUB <--- [chat, sender, message] -- Server PUB :3000
```

These are ZeroMQ multipart messages, not raw TCP messages. Each bracketed item
is a separate frame containing bytes.

| Exchange | Frames |
| --- | --- |
| Valid request | `[sender, message]` |
| Successful reply | `[ack]` |
| Invalid request reply | `[error: expected sender and message]` |
| Broadcast | `[chat, sender, message]` |

The server binds to `tcp://*:3000` and `tcp://*:3001`. Clients connect to
`tcp://localhost:3000` and `tcp://localhost:3001`. The wildcard is a bind address,
not a client destination.

### Topics and subscriptions

`chat` is the topic label in the first broadcast frame. A SUB socket subscribed
to `chat` receives matching broadcasts. ZeroMQ subscription matching uses a byte
prefix, not exact topic equality.

Both subscribed clients can receive a broadcast, including the sender. The
Python client explicitly ignores messages with its own sender name; the Rust
MVP does not yet implement that filter.

### Acceptance is not delivery

The server sends `ack` **before** publishing. An ACK means the server accepted
the request, not that publishing succeeded or that another client received it.
PUB/SUB does not provide a delivery acknowledgement. Startup broadcasts can be
missed while subscriptions are becoming active.

## Implementation milestones

### 1. Establish the Rust server structure

Started with the scaffolded “Hello, world!” program, added the `zmq` dependency,
and changed `main` to return `Result<(), zmq::Error>`.

Created a context, then PUB and REP sockets. Socket creation and binding return
`Result` values; `?` propagates failures from `main`. An early review caught
missing `?` operators on the bind calls.

`cargo check` checks compilation; only running the program attempts to bind
ports and can expose a port conflict.

### 2. Receive, acknowledge, and broadcast

Implemented one request before adding the server loop:

1. Receive multipart frames on REP.
2. Send a single-frame ACK.
3. Prepend the topic frame.
4. Publish the resulting multipart message.

Socket creation and binding stay outside the loop. Only per-message work
repeats. The infinite loop is the final expression, so no trailing `Ok(())` is
needed in the server.

### 3. Understand frames versus bytes

A request from Alice contained **two frames**, not 21 frames:

- `Alice`: 5 bytes.
- `hello from Alice`: 16 bytes.

The Rust receive type is `Vec<Vec<u8>>`. The outer vector counts frames; each
inner vector holds one frame's bytes. Thus, the outer length is 2, and the inner
lengths are 5 and 16.

An empty frame is different from a missing frame. `[chat, Alice, empty bytes]`
contains three frames; `[chat, Alice]` contains only two and breaks the Python
client's three-value unpacking.

### 4. Validate malformed requests

Added a guard requiring exactly two incoming frames. Invalid requests receive
an error reply and then take `continue`, skipping the normal ACK and broadcast.

The error reply matters because REP must reply before it can receive another
request. Likewise, REQ must alternate **send → receive → send → receive**.
Receiving a reply is a socket-state requirement, not a guarantee that the server
will remain available for the next request.

### 5. Build a Rust client

Added `src/bin/client.rs`, giving Cargo a second executable. The client first
sent a fixed greeting and printed the ACK, then added SUB reception.

Building the outgoing message exposed an important type distinction:

- `b"Alice"` references a fixed-size byte array.
- `vec![b"Alice"]` contains one array reference; it does not create a vector of
  individual bytes.
- `b"Alice".to_vec()` produces an owned `Vec<u8>`.

Using an outer `vec!` containing converted byte strings gives the intended
`Vec<Vec<u8>>`. No custom trait implementation or explicit iterator was needed.
A trait bound describes capabilities required of a type; supplying the correct
input shape satisfied the send API's requirements.

### 6. Receive concurrently with keyboard input

Moved SUB reception into a closure passed to `std::thread::spawn`:

| Thread | Work |
| --- | --- |
| Main | Read stdin, send REQ messages, receive replies |
| Receiver | Receive SUB broadcasts and print frames |

The closure uses `move` to take ownership of the SUB socket. `move` does not
start a thread; `spawn` does. Only captured values are transferred, so the REQ
socket remains available to `main`. Captured `Copy` values are copied.

The receive loop belongs **inside the closure**: one thread repeatedly receives
messages. A loop around `spawn` would instead attempt to create threads
repeatedly, and could not repeatedly move the same socket.

During the one-message exercise, joining demonstrated two error layers:
`join()` reports a thread panic, while the closure returns its own ZeroMQ
`Result`. Chaining `.expect(...)` and `?` handled those separately. `expect`
panics on failure; it is not recovery.

The final client does not join the indefinitely running receiver after EOF.
Returning from `main` ends the process and its background thread.

### 7. Read input and display text

The main loop reads into a fresh `String`, exits on a zero-byte read (EOF),
removes only trailing CR/LF characters, and converts the input to bytes. It sends
Alice's name and the input as two frames, then receives the reply before the
next iteration.

Client `main` returns `Result<(), Box<dyn std::error::Error>>`, allowing `?` to
propagate either stdin I/O errors or ZeroMQ errors.

For display, `String::from_utf8_lossy` converts each frame to readable text,
replacing invalid UTF-8 with `�`. ASCII is a subset of UTF-8. This changes only
display, not the bytes sent over the network.

## Running the lab

Run the following commands from the **repository root**. Use a current Rust
toolchain supporting edition 2024. The Python project requires Python 3.14 or
newer and `pyzmq`; the examples below use `uv` to resolve that environment.

Start the Rust server in one terminal:

```sh
cargo run --manifest-path chat-server/rust/Cargo.toml --bin rust
```

Start the Rust client in another:

```sh
cargo run --manifest-path chat-server/rust/Cargo.toml --bin client
```

Start Python's Bob client in a third:

```sh
uv run --project chat-server/python chat-server/python/client.py Bob
```

Once both clients are connected, type messages in each terminal. The Rust
client currently always identifies itself as `Alice`. It prints replies and
each broadcast frame on separate lines, including the topic and its own
broadcasts. It has no input prompt.

Use Ctrl+D at an empty stdin read to exit the Rust client on Unix, or Ctrl+C to
stop a program. A client waiting for a server reply may remain blocked rather
than reading further keyboard input.

### Malformed-request check

With the Rust server running and Bob subscribed:

```sh
uv run --project chat-server/python chat-server/python/test-invalid.py
```

Expected replies:

```text
[b'error: expected sender and message']
[b'ack']
```

The probe sends a one-frame request, receives its error, then sends a valid
request over the **same REQ socket**. Bob should receive `Still working` and
nothing from the malformed request. This is a manual probe with printed output,
not an assertion-based automated test.

## Verification results

The learner reported successful manual checks during the session:

- Python clients exchanged messages through the Rust server.
- Malformed requests received the expected error reply.
- A valid request succeeded on the same connection afterward.
- Bob received the valid message and no malformed broadcast.
- The Rust client received ACKs and broadcasts, then sent keyboard input.

At report time, this command also passed:

```sh
cargo check --manifest-path chat-server/rust/Cargo.toml --bins
```

It reported one warning: the standalone `_receiver;` expression in the client
drops the join handle. Dropping it detaches the thread; it does not join or stop
it. The behavior matches the MVP, but the redundant statement remains cleanup
work. The manual network checks were not rerun while writing this report.

## Threads versus Tokio

Tokio is an async runtime: roughly the role `asyncio` plays in Python. Rust's
`async` and `.await` syntax needs a runtime to schedule tasks and drive async
I/O. Many async tasks can share a small pool of OS threads.

Our `zmq` operations are blocking. Putting them inside an `async` function would
not make them nonblocking; it would block the runtime worker executing them.
An async design would need an appropriate ZeroMQ integration or a deliberate
bridge to blocking threads. Python's keyboard-reading code similarly uses
`asyncio.to_thread` for blocking input.

Two OS threads were the smaller solution for this client. Tokio is not
automatically faster or necessary for a working chat MVP.

## Current limits and possible next steps

This is a learning example, not a production chat service:

- No authentication, encryption, or delivery guarantee. The server binds all
  interfaces, so use a trusted environment or restrict its bind addresses.
- Validation checks frame count only, not identity, text encoding, or size.
- No request/reply timeouts; a missing server can leave a client waiting.
- Receiver-thread errors are not monitored by the main thread.
- No coordinated graceful shutdown or automated regression suite.
- The Rust client has a fixed name, no startup greeting, no self-message filter,
  and basic frame-by-frame output.

Useful next exercises are configurable names, `sender: message` formatting,
self-message filtering, reply/error handling, and repeatable protocol tests.
