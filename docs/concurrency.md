# Concurrency

`lunelle-core::Runtime` runs `FnOnce(CancellationToken)` jobs on Crossbeam-backed worker queues. Jobs own the data they need; do not pass mutable component trees to workers. A `Task<T>` supports cooperative cancellation and nonblocking result checks. Cancellation is advisory: a worker must check its token and stop its own work.

Callbacks queued with `Runtime::schedule_on_ui` run only when the thread that created the runtime calls `process_ui_tasks`. This drain is nonblocking and executes callbacks without holding the queue lock. `spawn_with_completion` and `spawn_with_completion_typed` invoke their completion callback on a worker thread after publishing the task result. Use `spawn_with_ui_completion` or `spawn_with_ui_completion_typed` when completion must run on the runtime's owner thread; those APIs enqueue the callback, so the owner must continue draining the queue. For other worker outputs, send owned values through `crossbeam-channel` and use nonblocking receives on the UI thread. Attach a snapshot/version to results that may become stale and discard results whose version no longer matches. The runtime does not automatically marshal arbitrary task return values or enforce version checks.

Dropping `Runtime` marks outstanding tasks cancelled, sends worker shutdown messages, and joins workers. Consequently, workers must cooperate and must not block indefinitely in non-cancellable I/O if prompt shutdown is required.

Run the backend-neutral worker example with:

```sh
cargo run --example background_task --no-default-features
```

The example sends versioned results through the UI queue and discards the stale version on the owner thread. Its yield loop is illustrative; a window application should wake its event loop when a result arrives rather than spin.