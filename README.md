# Lee Interview Prep 2026

## URL shortener — first example

[`url_shortener/interview_url_shortener.rs`](url_shortener/interview_url_shortener.rs)
ports [`interview_url_shortener.py`](../termie-interviewprep-2026-10/url_shortener/interview_url_shortener.py),
the first example listed in the original URL shortener guide.

Requires a current Rust toolchain. The URL shortener uses only the standard library.

```sh
cd lee-interviewprep-2026
cargo run --bin interview_url_shortener
cargo test
```

Includes Base62 encoding/decoding, sequential IDs starting at 100,000, idempotent
shortening, custom aliases with collision errors, lazy TTL expiration, and the
original demo. `shorten` takes an original URL, an optional alias, and an optional
TTL in seconds. As in Python, zero TTL means no expiration; negative TTL expires
immediately. `resolve` takes a code, not the full short URL.

Two source edge cases are corrected: generated codes skip occupied custom
aliases, and expiring an old code or alias does not delete a newer URL mapping.
Base62 decoding reports invalid characters and values larger than `u64`.

This is an in-memory interview example, not an HTTP server: data disappears on
exit, codes are predictable, and URLs/aliases are not sanitized for web use.

## Rate limiters — first example

[`rate_limiters/async_token_bucket.rs`](rate_limiters/async_token_bucket.rs)
ports [`async_token_bucket.py`](../termie-interviewprep-2026-10/rate_limiters/async_token_bucket.py),
the first example in the original rate limiter guide. Uses Tokio for async
locking, timers, and concurrent requests.

```sh
# From lee-interviewprep-2026/
cargo run --bin async_token_bucket
cargo test --bin async_token_bucket
```

Includes continuous token refill, burst capacity (defaults to the rate),
fractional token requests, waiting and denying modes, per-request mode overrides,
and denials carrying `retry_after` in seconds. The demo runs six concurrent
requests in each mode: three burst immediately; the remainder are denied or
wait for refill.

- `AsyncTokenBucket::new(rate, capacity, blocking)` constructs the limiter.
- `acquire(tokens, blocking).await` consumes tokens or waits/reports a denial.
- `try_acquire(tokens).await` returns `Ok(false)` immediately when exhausted.
- `limit(future, blocking).await` acquires one token before polling the future,
  replacing Python's decorator. In place of `async with`, call
  `acquire(1.0, None).await?` before the operation; tokens are not returned afterward.

`None` uses the configured default for capacity or blocking. Invalid/non-finite
rates, capacities, and token requests return errors. Unlike the Python source,
waiting releases the mutex so fail-fast requests cannot get stuck behind a
sleeping caller. Waiters compete on refill; strict FIFO fairness is not promised.
Tests use Tokio's paused clock rather than real sleeps.

State is process-local, not a shared/distributed quota. The other rate limiter
examples have not been ported.

## Distributed cron — first example

[`distributed_cron/interview_distributed_cron.rs`](distributed_cron/interview_distributed_cron.rs)
ports [`interview_distributed_cron.py`](../termie-interviewprep-2026-10/distributed_cron/interview_distributed_cron.py),
the first example in the original distributed cron guide. Reuses Tokio; no new
dependencies.

```sh
# From lee-interviewprep-2026/
cargo run --bin interview_distributed_cron
cargo test --bin interview_distributed_cron
```

Includes a shared heartbeat lease store, a min-heap schedule, concurrent async
job dispatch, periodic rescheduling, and a two-node failover demo. The leader
renews a 400 ms lease while polling every 50 ms; followers poll every 100 ms.
Stopping node-1's heartbeats leaves its lease to expire before node-2 takes over.

- `DistributedCronNode::new(node_id, store)` shares an `Arc<Mutex<DistributedLeaseStore>>`.
- `add_job(job_id, interval, action)` accepts a `Duration` and an async action
  factory, such as `|| async { /* work */ }`. Zero/overflowing intervals are rejected.
- `run(shutdown).await` schedules until a watch channel signals `true` or closes.
  Graceful shutdown finishes dispatched jobs; aborting the scheduler cancels them.

Like the Python implementation, missed ticks collapse to one dispatch followed
by rescheduling from the current time. Long jobs do not block scheduling and can
overlap later runs. Task panics propagate as scheduler errors. The Rust port uses
monotonic time, atomic lease access under a mutex, and treats the expiry instant
as expired. Tests use Tokio's paused clock for scheduling and crash/takeover checks.

This is a single-process simulation, not a real coordination service. Despite
its Python docstring, the source does not implement dispatch deduplication; this
port likewise provides neither fencing nor exactly-once execution. The other
distributed cron examples have not been ported.

## Job queues — first example

[`job_queues/async_job_queue.rs`](job_queues/async_job_queue.rs)
ports [`async_job_queue.py`](../termie-interviewprep-2026-10/job_queues/async_job_queue.py),
the first example in the original job queue guide. Reuses Tokio; no new dependencies.

```sh
# From lee-interviewprep-2026/
cargo run --bin async_job_queue
cargo test --bin async_job_queue
```

Includes a FIFO ready queue, concurrent worker pool, delayed retry min-heap,
exponential backoff with +/-20% jitter, observable job status/results/errors,
and a dead-letter queue. A background scheduler starts on construction and
promotes retries at their deadlines without tying up workers. The demo submits
clean, flaky, and permanently failing jobs.

- `AsyncJobQueue::<T>::new(num_workers)` requires a Tokio runtime and at least
  one worker. Jobs in a queue return the same result type `T`.
- `enqueue(action, max_retries, base_delay, max_delay)` returns a shared job handle.
  The action is a reusable async factory returning `Result<T, String>`; capture
  arguments in its closure. Retry delays use `Duration`; the Python defaults are
  `3`, `Duration::from_millis(500)`, and `Duration::from_secs(30)`.
- `join().await` waits for all submitted jobs, including delayed retries, to finish.
- `dead_letter_queue().await` returns a copy of the failed-job handle list.
- `stop().await` consumes the queue and gracefully drains jobs and retries. Unlike
  Python's cancelling stop, it does not discard queued work. Dropping the queue
  or cancelling shutdown aborts its background tasks.

`max_retries` excludes the initial attempt: two retries permit three attempts.
As in Python, the backoff is capped *before* jitter, so a delay can reach 120% of
`max_delay`. Non-security jitter uses a randomly seeded standard-library hash.
IDs are queue-local counters rather than UUIDs. Worker panics surface as queue
errors rather than hanging `join`; ordinary `Err` results are retried. Tests use
Tokio's paused clock.

This queue is unbounded, process-local, and non-durable; abrupt shutdown loses
work, and graceful shutdown waits indefinitely for jobs that never finish.
The SQLite and other job queue examples have not been ported.

## Client/server — first example

[`client_server/client_server_raw_tcp.rs`](client_server/client_server_raw_tcp.rs)
ports [`client_server_raw_tcp.py`](../termie-interviewprep-2026-10/client_server/client_server_raw_tcp.py),
the first example in the original networking guide. Uses only the standard library.

```sh
# From lee-interviewprep-2026/
cargo run --bin client_server_raw_tcp
cargo test --bin client_server_raw_tcp
```

The demo binds `127.0.0.1:9999`, starts a thread-per-client TCP server, sends
`ping`, `hello world`, and `interview test`, then stops accepting connections.
The listener is bound before starting clients, avoiding Python's startup sleep.
Port 9999 must be available; tests use OS-assigned localhost ports.

Messages are newline-delimited UTF-8. Whitespace is stripped, replies are
`ACK: <message>`, and blank messages or `QUIT` close the connection without a
reply. Connections support multiple requests; buffered reading handles split
and coalesced TCP writes. Like Python, an unterminated final message at EOF is
also processed.

- `handle_client(conn, addr)` handles one connection.
- `run_server(listener, stop)` accepts until the shared atomic stop flag is set.
- `send_request(address, message)` connects, sends one request, and reads a reply.

The port limits request frames to 64 KiB including the newline (and allows room
for the reply's `ACK: ` prefix), rejects invalid UTF-8 and multiline client input,
and sets five-second socket I/O/connect timeouts. Client errors are reported
without stopping the listener. As in Python, client handler threads are detached;
stopping the listener does not join active handlers. Idle handlers time out.

This is a localhost interview demo, not a hardened server: there is no TLS,
authentication, or connection/thread limit. The ZeroMQ, WebSocket, and asyncio
streams examples have not been ported.
