use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use thiserror::Error;

thread_local! { static ACTIVE_SIGNAL_DISPATCH: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) }; }
static NEXT_SIGNAL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Error)]
pub enum CoreError {
    #[error("signal lock was poisoned")]
    SignalPoisoned,
}

/// Shared signal. Reads and writes are synchronized and may be called from any thread
/// for which `T` is `Send`. Subscribers run synchronously on the thread calling `set`,
/// in registration order, from a snapshot taken at the start of that notification pass.
/// Subscriptions added or removed during a pass affect the next pass. Recursive updates
/// to this signal change its value but do not start another pass. A subscription does
/// not keep the signal alive and becomes inert when the signal is dropped.
struct SignalInner<T> {
    id: u64,
    value: Mutex<T>,
    subscribers: Mutex<Vec<(u64, Arc<dyn Fn() + Send + Sync>)>>,
    next_subscriber: AtomicU64,
}

/// Thread-safe shared value with synchronous change notifications.
#[derive(Clone)]
pub struct Signal<T> {
    inner: Arc<SignalInner<T>>,
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(SignalInner {
                id: NEXT_SIGNAL_ID.fetch_add(1, Ordering::Relaxed),
                value: Mutex::new(value),
                subscribers: Mutex::new(Vec::new()),
                next_subscriber: AtomicU64::new(1),
            }),
        }
    }

    pub fn get(&self) -> Result<T, CoreError> {
        Ok(self
            .inner
            .value
            .lock()
            .map_err(|_| CoreError::SignalPoisoned)?
            .clone())
    }

    /// Replaces the value, then synchronously notifies the current subscriber snapshot.
    /// Recursive same-signal writes update the value without re-entering callbacks.
    pub fn set(&self, value: T) -> Result<(), CoreError> {
        *self
            .inner
            .value
            .lock()
            .map_err(|_| CoreError::SignalPoisoned)? = value;
        let already_dispatching =
            ACTIVE_SIGNAL_DISPATCH.with(|active| active.borrow().contains(&self.inner.id));
        if already_dispatching {
            return Ok(());
        }
        let _guard = SignalDispatchGuard::enter(self.inner.id);
        let subscribers = self
            .inner
            .subscribers
            .lock()
            .map_err(|_| CoreError::SignalPoisoned)?
            .clone();
        for (_, callback) in subscribers {
            callback();
        }
        Ok(())
    }

    /// Registers a callback and returns its owned cancellation handle.
    pub fn subscribe(
        &self,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Result<Subscription<T>, CoreError> {
        let id = self.inner.next_subscriber.fetch_add(1, Ordering::Relaxed);
        self.inner
            .subscribers
            .lock()
            .map_err(|_| CoreError::SignalPoisoned)?
            .push((id, Arc::new(callback)));
        Ok(Subscription {
            inner: Arc::downgrade(&self.inner),
            id,
        })
    }
}

struct SignalDispatchGuard(u64);

impl SignalDispatchGuard {
    fn enter(id: u64) -> Self {
        ACTIVE_SIGNAL_DISPATCH.with(|active| active.borrow_mut().push(id));
        Self(id)
    }
}

impl Drop for SignalDispatchGuard {
    fn drop(&mut self) {
        ACTIVE_SIGNAL_DISPATCH.with(|active| {
            let index = { active.borrow().iter().rposition(|id| *id == self.0) };
            if let Some(index) = index {
                active.borrow_mut().remove(index);
            }
        });
    }
}

/// Removes a signal callback when dropped. It is safe to move across threads and does
/// not keep the signal alive; dropping the signal makes this handle inert.
pub struct Subscription<T> {
    inner: std::sync::Weak<SignalInner<T>>,
    id: u64,
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            if let Ok(mut subscribers) = inner.subscribers.lock() {
                subscribers.retain(|(id, _)| *id != self.id);
            }
        }
    }
}
