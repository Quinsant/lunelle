# Signals and State

`lunelle-core::Signal<T>` stores a cloneable value behind shared ownership. `get()` returns a clone and `set()` replaces the value. Subscribers run synchronously, in registration order, on the thread that called `set`; callbacks are not automatically moved to the UI thread. Keep UI state updates on the UI owner thread, or use `TaskRuntime::schedule_on_ui` to queue them for `Runtime::process_ui_tasks` on the runtime owner thread.

`subscribe()` returns a `Subscription`; dropping it removes that callback. A callback may update the same signal: the value changes immediately, but a recursive update does not begin a nested notification pass. A subscription can be moved across threads, so dropping it there is supported; signal callbacks themselves still execute on the thread calling `set`.

For asynchronous derived state, capture an immutable input snapshot and its version before starting work. Return an owned result and apply it on the UI thread only if the version still matches. See [concurrency.md](concurrency.md) for runtime and channel ownership rules.