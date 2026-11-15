//! Backend-neutral contracts and shared types for Lunelle framework.

pub mod diagnostics;
pub mod geometry;
pub mod message;
pub mod render;
#[cfg(feature = "render-wgpu")]
pub mod render_wgpu;
pub mod resource;
pub mod runtime;
pub mod signal;
pub mod task;
pub mod window;
#[cfg(feature = "window-winit")]
pub mod window_winit;
pub use diagnostics::{ResourceError, UiError};
pub use geometry::{ComponentId, ElementId, NodeId, Point, Rect, ScaleFactor, Size, WindowId};
pub use message::{MessageClosed, MessageQueue, MessageReceiver, MessageSender, TryReceiveError};
pub use render::{
    DrawCommand, FrameDrawCommand, FrameRenderer, RenderError, RenderFrame, RenderTargetInfo,
    Renderer,
};
pub use resource::{
    ImageData, Resource, ResourceCache, ResourceKey, ResourceKind, ResourceLoader, ResourceRoot,
    ResourceTable,
};
pub use runtime::Runtime;
pub use signal::{CoreError, Signal, Subscription};
pub use task::{CancellationToken, Task, TaskControl, TaskError, TaskPoll, TaskRuntime};
pub use window::{MouseButton, TouchPhase, WindowBackend, WindowEvent, WindowOptions};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color(pub [f32; 4]);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::Ordering};
    use std::thread;
    use task::{Runtime, TaskControl, TaskRuntime};

    #[test]
    fn invalid_scale_factor_is_rejected() {
        assert!(ScaleFactor::new(0.0).is_none());
    }
    #[test]
    fn ui_tasks_are_drained_by_owner() {
        let runtime = Runtime::new(1);
        let value = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cloned = value.clone();
        runtime
            .schedule_on_ui(Box::new(move || cloned.store(true, Ordering::Release)))
            .unwrap();
        assert_eq!(runtime.process_ui_tasks().unwrap(), 1);
        assert!(value.load(Ordering::Acquire));
    }
    #[test]
    fn ui_callbacks_can_drain_the_queue_recursively() {
        let runtime = Arc::new(Runtime::new(1));
        let nested_runtime = runtime.clone();
        let processed = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let nested_processed = processed.clone();
        runtime
            .schedule_on_ui(Box::new(move || {
                nested_processed.fetch_add(1, Ordering::Relaxed);
                assert_eq!(nested_runtime.process_ui_tasks().unwrap(), 1);
            }))
            .unwrap();
        let nested_processed = processed.clone();
        runtime
            .schedule_on_ui(Box::new(move || {
                nested_processed.fetch_add(1, Ordering::Relaxed);
            }))
            .unwrap();

        assert_eq!(runtime.process_ui_tasks().unwrap(), 1);
        assert_eq!(processed.load(Ordering::Relaxed), 2);
    }
    #[test]
    fn worker_completion_can_be_scheduled_on_the_ui_owner_thread() {
        let runtime = Runtime::new(1);
        let owner_thread = thread::current().id();
        let (completed_tx, completed_rx) = crossbeam_channel::bounded(1);
        let task = runtime
            .spawn_with_ui_completion(
                |_| Ok(()),
                move || completed_tx.send(thread::current().id()).unwrap(),
            )
            .unwrap();
        while !task.is_finished() {
            thread::yield_now();
        }

        assert!(matches!(
            completed_rx.try_recv(),
            Err(crossbeam_channel::TryRecvError::Empty)
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while runtime.process_ui_tasks().unwrap() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "UI completion was not queued"
            );
            thread::yield_now();
        }
        assert_eq!(completed_rx.recv().unwrap(), owner_thread);
    }
    #[test]
    fn typed_worker_completion_is_scheduled_on_the_ui_owner_thread() {
        let runtime = Runtime::new(1);
        let owner_thread = thread::current().id();
        let (completed_tx, completed_rx) = crossbeam_channel::bounded(1);
        let task = runtime
            .spawn_with_ui_completion_typed(
                |_| Ok::<_, std::io::Error>(()),
                move || completed_tx.send(thread::current().id()).unwrap(),
            )
            .unwrap();
        while !task.is_finished() {
            thread::yield_now();
        }

        assert!(matches!(
            completed_rx.try_recv(),
            Err(crossbeam_channel::TryRecvError::Empty)
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while runtime.process_ui_tasks().unwrap() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "UI completion was not queued"
            );
            thread::yield_now();
        }
        assert_eq!(completed_rx.recv().unwrap(), owner_thread);
    }
    #[test]
    fn ui_drain_does_not_wait_for_workers_and_stale_results_are_ignored() {
        let runtime = Arc::new(Runtime::new(1));
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let (release_tx, release_rx) = crossbeam_channel::bounded(1);
        let worker_runtime = runtime.clone();
        let current_version = 8;
        let result_version = current_version - 1;
        let applied = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_applied = applied.clone();
        let task = runtime
            .spawn(move |_| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let _ = worker_runtime.schedule_on_ui(Box::new(move || {
                    if result_version == current_version {
                        worker_applied.store(true, Ordering::Release);
                    }
                }));
                Ok(())
            })
            .unwrap();
        started_rx.recv().unwrap();

        let ui_ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ui_callback_ran = ui_ran.clone();
        runtime
            .schedule_on_ui(Box::new(move || {
                ui_callback_ran.store(true, Ordering::Release)
            }))
            .unwrap();
        assert_eq!(runtime.process_ui_tasks().unwrap(), 1);
        assert!(ui_ran.load(Ordering::Acquire));
        assert!(!task.is_finished());

        release_tx.send(()).unwrap();
        while !task.is_finished() {
            thread::yield_now();
        }
        assert_eq!(runtime.process_ui_tasks().unwrap(), 1);
        assert!(!applied.load(Ordering::Acquire));
    }
    #[test]
    fn dropping_subscription_stops_notifications() {
        let signal = Signal::new(1_u8);
        let count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let captured = count.clone();
        let subscription = signal
            .subscribe(move || {
                captured.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        signal.set(2).unwrap();
        drop(subscription);
        signal.set(3).unwrap();
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn signal_notifies_subscribers_in_registration_order() {
        let signal = Signal::new(());
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let first_order = order.clone();
        let _first = signal
            .subscribe(move || first_order.lock().unwrap().push(1))
            .unwrap();
        let second_order = order.clone();
        let _second = signal
            .subscribe(move || second_order.lock().unwrap().push(2))
            .unwrap();

        signal.set(()).unwrap();

        assert_eq!(*order.lock().unwrap(), [1, 2]);
    }
    #[test]
    fn task_cancellation_is_cooperative_and_completion_is_observable() {
        let runtime = Runtime::new(1);
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let task = runtime
            .spawn(move |token| {
                started_tx.send(()).unwrap();
                while !token.is_cancelled() {
                    thread::yield_now();
                }
                Ok(())
            })
            .unwrap();
        started_rx.recv().unwrap();
        task.cancel();
        while !task.is_finished() {
            thread::yield_now();
        }
        assert!(task.is_finished());
    }
    #[test]
    fn runtime_spawn_returns_a_cancellable_task_handle() {
        let runtime = Runtime::new(1);
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let handle = runtime
            .spawn(move |_| {
                started_tx.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        started_rx.recv().unwrap();
        while !handle.is_finished() {
            thread::yield_now();
        }
        assert!(handle.is_finished());
    }
    #[test]
    fn recursive_signal_updates_do_not_reenter_subscribers() {
        let signal = Arc::new(Signal::new(0_u8));
        let observed = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let captured = observed.clone();
        let nested = signal.clone();
        let _subscription = signal
            .subscribe(move || {
                captured.fetch_add(1, Ordering::Relaxed);
                nested.set(2).unwrap();
            })
            .unwrap();
        signal.set(1).unwrap();
        assert_eq!(signal.get().unwrap(), 2);
        assert_eq!(observed.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn dropping_runtime_cancels_and_joins_workers() {
        let runtime = Runtime::new(1);
        let (started_tx, started_rx) = crossbeam_channel::bounded(1);
        let (finished_tx, finished_rx) = crossbeam_channel::bounded(1);
        runtime
            .spawn(move |token| {
                started_tx.send(()).unwrap();
                while !token.is_cancelled() {
                    thread::yield_now();
                }
                finished_tx.send(()).unwrap();
                Ok(())
            })
            .unwrap();
        started_rx.recv().unwrap();
        drop(runtime);
        assert!(finished_rx.try_recv().is_ok());
    }
}
