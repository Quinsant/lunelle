use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use crate::task::{CancellationToken, Task, TaskError, TaskRuntime};
use crossbeam_channel::{self as channel, Receiver, Sender, TryRecvError};

pub struct Runtime {
    background: Option<tokio::runtime::Runtime>,
    ui_sender: Sender<Box<dyn FnOnce() + Send + 'static>>,
    ui_receiver: Mutex<Receiver<Box<dyn FnOnce() + Send + 'static>>>,
    ui_thread: thread::ThreadId,
    ui_waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    cancellations: Arc<Mutex<Vec<std::sync::Weak<AtomicBool>>>>,
}

impl Runtime {
    pub fn new(worker_count: usize) -> Self {
        let worker_count = worker_count.max(1);
        let background = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .max_blocking_threads(worker_count)
            .thread_name("lunelle-background")
            .build()
            .expect("failed to create Tokio background runtime");
        let (ui_sender, ui_receiver) = channel::unbounded();
        Self {
            background: Some(background),
            ui_sender,
            ui_receiver: Mutex::new(ui_receiver),
            ui_thread: thread::current().id(),
            ui_waker: Arc::new(Mutex::new(None)),
            cancellations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn set_ui_waker(&self, waker: Option<Arc<dyn Fn() + Send + Sync>>) {
        if let Ok(mut current) = self.ui_waker.lock() {
            *current = waker;
        }
    }

    fn enqueue_ui(
        sender: &Sender<Box<dyn FnOnce() + Send>>,
        waker: &Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
        task: Box<dyn FnOnce() + Send>,
    ) -> Result<(), TaskError> {
        sender
            .send(task)
            .map_err(|error| TaskError::Failed(error.to_string()))?;
        let wake = waker.lock().ok().and_then(|waker| waker.clone());
        if let Some(wake) = wake {
            wake();
        }
        Ok(())
    }

    /// Runs a CPU-bound task on the bounded pool reserved for blocking computation.
    pub fn spawn_css<T, F>(&self, work: F) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
    {
        self.spawn(work)
    }

    /// Starts a background future on the shared Tokio runtime.
    ///
    /// Tokio tasks use their native join handle; CPU-bound work must use `spawn_css`.
    pub fn spawn_background<T, F>(&self, future: F) -> tokio::task::JoinHandle<Result<T, TaskError>>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        self.background
            .as_ref()
            .expect("background runtime has stopped")
            .spawn(future)
    }

    /// Runs a background future and queues its completion callback for the UI owner.
    pub fn spawn_background_with_ui_completion<T, F, C>(
        &self,
        future: F,
        completion: C,
    ) -> tokio::task::JoinHandle<Result<T, TaskError>>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, TaskError>> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        let ui_sender = self.ui_sender.clone();
        let ui_waker = self.ui_waker.clone();
        self.spawn_background(async move {
            let result = future.await;
            let _ = Self::enqueue_ui(&ui_sender, &ui_waker, Box::new(completion));
            result
        })
    }

    /// Runs queued callbacks on the thread that calls this method.
    pub fn process_ui_tasks(&self) -> Result<usize, TaskError> {
        if thread::current().id() != self.ui_thread {
            return Err(TaskError::Failed(
                "UI callbacks must be processed on the runtime owner thread".into(),
            ));
        }
        let mut processed = 0;
        loop {
            let task = {
                let Ok(receiver) = self.ui_receiver.lock() else {
                    return Err(TaskError::Failed("UI queue lock poisoned".into()));
                };
                match receiver.try_recv() {
                    Ok(task) => Some(task),
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
                }
            };
            let Some(task) = task else {
                return Ok(processed);
            };
            task();
            processed += 1;
        }
    }

    pub fn spawn<T, F>(&self, work: F) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
    {
        self.spawn_with_completion(work, || {})
    }

    pub fn spawn_typed<T, F, E>(&self, work: F) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, E> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.spawn_with_completion_typed(work, || {})
    }

    pub fn spawn_with_completion<T, F, C>(
        &self,
        work: F,
        completion: C,
    ) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        self.spawn_inner(
            move |token| work(token).map_err(TaskError::Failed),
            completion,
        )
    }

    /// Spawns worker work and queues its completion callback for `process_ui_tasks`.
    /// The callback is never invoked inline by the worker thread.
    pub fn spawn_with_ui_completion<T, F, C>(
        &self,
        work: F,
        completion: C,
    ) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        let ui_sender = self.ui_sender.clone();
        let ui_waker = self.ui_waker.clone();
        self.spawn_with_completion(work, move || {
            let _ = Self::enqueue_ui(&ui_sender, &ui_waker, Box::new(completion));
        })
    }

    pub fn spawn_with_completion_typed<T, F, C, E>(
        &self,
        work: F,
        completion: C,
    ) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, E> + Send + 'static,
        C: FnOnce() + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.spawn_inner(
            move |token| {
                work(token).map_err(|error| TaskError::caused("worker operation failed", error))
            },
            completion,
        )
    }

    /// Spawns typed worker work and queues completion for `process_ui_tasks`.
    pub fn spawn_with_ui_completion_typed<T, F, C, E>(
        &self,
        work: F,
        completion: C,
    ) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, E> + Send + 'static,
        C: FnOnce() + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let ui_sender = self.ui_sender.clone();
        let ui_waker = self.ui_waker.clone();
        self.spawn_with_completion_typed(work, move || {
            let _ = Self::enqueue_ui(&ui_sender, &ui_waker, Box::new(completion));
        })
    }

    fn spawn_inner<T, F, C>(&self, work: F, completion: C) -> Result<Task<T>, TaskError>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Result<T, TaskError> + Send + 'static,
        C: FnOnce() + Send + 'static,
    {
        let cancelled = Arc::new(AtomicBool::new(false));
        if let Ok(mut cancellations) = self.cancellations.lock() {
            cancellations.retain(|cancellation| cancellation.strong_count() > 0);
            cancellations.push(Arc::downgrade(&cancelled));
        }
        let token = CancellationToken(cancelled.clone());
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let (sender, result) = channel::bounded(1);
        let job = Box::new(move || {
            let outcome = if token.is_cancelled() {
                Err(TaskError::Cancelled)
            } else {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(token.clone())))
                {
                    Ok(result) => result,
                    Err(payload) => Err(TaskError::Failed(panic_message(payload.as_ref()))),
                }
            };
            let outcome = if token.is_cancelled() {
                Err(TaskError::Cancelled)
            } else {
                outcome
            };
            let _ = sender.send(outcome);
            worker_finished.store(true, Ordering::Release);
            completion();
        });
        self.background
            .as_ref()
            .expect("background runtime has stopped")
            .spawn_blocking(job);
        Ok(Task {
            cancelled,
            finished,
            result: Mutex::new(result),
        })
    }
}

impl TaskRuntime for Runtime {
    type TaskHandle = Task<()>;
    type Error = TaskError;

    fn spawn(
        &self,
        task: Box<dyn FnOnce(CancellationToken) + Send + 'static>,
    ) -> Result<Self::TaskHandle, Self::Error> {
        Runtime::spawn(self, move |cancellation| {
            task(cancellation);
            Ok(())
        })
    }

    fn schedule_on_ui(&self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), Self::Error> {
        Self::enqueue_ui(&self.ui_sender, &self.ui_waker, task)
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else {
        "background task panicked".to_owned()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Ok(cancellations) = self.cancellations.lock() {
            for cancellation in cancellations.iter().filter_map(std::sync::Weak::upgrade) {
                cancellation.store(true, Ordering::Release);
            }
        }
        if let Some(background) = self.background.take() {
            background.shutdown_background();
        }
    }
}
