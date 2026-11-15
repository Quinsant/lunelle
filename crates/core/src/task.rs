use crossbeam_channel::{Receiver, TryRecvError};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

pub use crate::runtime::Runtime;

/// Controls cancellation and completion without exposing worker implementation details.
/// Handles are shareable across threads; cancellation is cooperative.
pub trait TaskControl: Send + Sync {
    fn cancel(&self);
    fn is_finished(&self) -> bool;
}

/// Runtime boundary for worker jobs and UI-thread scheduling.
/// Implementations are shareable across threads, must enqueue UI work, and must never
/// mutate the UI tree from a worker. The task closure is transferred to the runtime;
/// the UI closure is transferred to the UI queue and runs when that queue is drained.
pub trait TaskRuntime: Send + Sync {
    type TaskHandle: TaskControl;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Transfers a one-shot job to a worker and returns its owned control handle.
    fn spawn(
        &self,
        task: Box<dyn FnOnce(CancellationToken) + Send + 'static>,
    ) -> Result<Self::TaskHandle, Self::Error>;

    /// Transfers a one-shot callback to the UI queue; this method must not run it inline
    /// on a worker thread.
    fn schedule_on_ui(&self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Error)]
pub enum TaskError {
    #[error("task was cancelled")]
    Cancelled,
    #[error("task failed: {0}")]
    Failed(String),
    #[error("task failed: {message}: {source}")]
    Caused {
        message: String,
        #[source]
        source: SharedError,
    },
}

#[derive(Clone)]
pub struct SharedError(Arc<dyn std::error::Error + Send + Sync>);

impl std::fmt::Debug for SharedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("SharedError").field(&self.0).finish()
    }
}

impl std::fmt::Display for SharedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for SharedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

impl std::ops::Deref for SharedError {
    type Target = dyn std::error::Error + Send + Sync;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl PartialEq for TaskError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Cancelled, Self::Cancelled) => true,
            (Self::Failed(left), Self::Failed(right)) => left == right,
            (
                Self::Caused {
                    message: left_message,
                    source: left_source,
                },
                Self::Caused {
                    message: right_message,
                    source: right_source,
                },
            ) => {
                left_message == right_message && left_source.to_string() == right_source.to_string()
            }
            _ => false,
        }
    }
}

impl Eq for TaskError {}

impl TaskError {
    pub(crate) fn caused(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Caused {
            message: message.into(),
            source: SharedError(Arc::new(source)),
        }
    }
}

#[derive(Clone)]
pub struct CancellationToken(pub(crate) Arc<AtomicBool>);

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub enum TaskPoll<T> {
    Pending,
    Ready(Result<T, TaskError>),
}

pub struct Task<T> {
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) finished: Arc<AtomicBool>,
    pub(crate) result: Mutex<Receiver<Result<T, TaskError>>>,
}

impl<T> Task<T> {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn try_take(&self) -> TaskPoll<T> {
        let Ok(result) = self.result.lock() else {
            return TaskPoll::Ready(Err(TaskError::Failed("task result lock poisoned".into())));
        };
        match result.try_recv() {
            Ok(result) => TaskPoll::Ready(result),
            Err(TryRecvError::Empty) => TaskPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                TaskPoll::Ready(Err(TaskError::Failed("worker stopped unexpectedly".into())))
            }
        }
    }
}

impl<T: Send + 'static> TaskControl for Task<T> {
    fn cancel(&self) {
        Task::cancel(self);
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}
