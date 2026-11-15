//! Application Object Model: UI-thread-owned component and immutable style snapshots.
pub mod accessibility;
#[cfg(feature = "base_component")]
pub mod base;
pub mod component;
pub mod diff;
pub mod event;
pub mod tree;
pub use accessibility::{Accessibility, AccessibilityRole, FocusManager, FocusableTree};
#[cfg(feature = "base_component")]
pub use base::{BuiltinComponent, Button, Div, Input, Li, List, P, Text};
pub use component::{Component, ComponentHost, ComponentInstance, ComponentInstanceId};
pub use event::{
    Event, EventDispatcher, EventPhase, EventTree, UiAction, UiEvent, dispatch_drag_cancel,
    dispatch_mouse_release, dispatch_pointer_events,
};
pub use tree::{AomError, Element, ElementData, IntoView, StyleNode, StyleResult, StyleSnapshot};
#[cfg(all(test, feature = "base_component"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static LIFECYCLE: AtomicUsize = AtomicUsize::new(0);
    static PANIC_UNMOUNTS: AtomicUsize = AtomicUsize::new(0);
    static LIFECYCLE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    struct Counter(usize);
    impl Component for Counter {
        type Props = usize;
        type Message = usize;
        fn create(props: usize) -> Self {
            Self(props)
        }
        fn update(&mut self, message: usize) {
            self.0 += message;
        }
        fn view(&self) -> Box<dyn Element> {
            Box::new(P::new().with_text(self.0.to_string()))
        }
        fn on_mount(&mut self) {
            LIFECYCLE.fetch_add(1, Ordering::Relaxed);
        }
        fn on_unmount(&mut self) {
            LIFECYCLE.fetch_add(10, Ordering::Relaxed);
        }
    }
    #[test]
    fn snapshots_keep_stable_ids_and_parent_links() {
        let root = Div::new().child(Button::new());
        let first = root.style_snapshot(7);
        let second = root.style_snapshot(8);
        assert_eq!(first.nodes[0].id, second.nodes[0].id);
        assert_eq!(first.nodes[1].parent, Some(first.nodes[0].id));
    }

    #[test]
    fn class_attribute_splits_whitespace_separated_names() {
        let snapshot = Div::new()
            .with_class("flex flex-col gap-4")
            .style_snapshot(1);

        assert_eq!(snapshot.nodes[0].classes, ["flex", "flex-col", "gap-4"]);
    }
    #[test]
    fn stale_style_results_are_rejected() {
        let snapshot = Div::new().style_snapshot(12);
        assert!(snapshot.accepts_result(11).is_err());
        assert!(snapshot.accepts_result(12).is_ok());
    }
    #[test]
    fn style_snapshot_computes_and_versions_css_result() {
        let root = Div::new().with_class("theme").child(Button::new());
        let snapshot = root.style_snapshot(42);
        let css = lunelle_css::CssRender::parse(".theme { color: navy; }");
        let (result, diagnostics) = snapshot.compute(&css);
        assert!(diagnostics.is_empty());
        assert_eq!(result.version, 42);
        assert_eq!(result.stylesheet_version, 0);
        assert_eq!(result.environment_version, 0);
        assert_eq!(result.styles[1].properties["color"], "navy");
    }

    #[test]
    fn computed_result_rejects_stale_stylesheet_and_environment_versions() {
        let snapshot = Div::new().style_snapshot(42);
        let css = lunelle_css::CssRender::parse("main { color: navy; }");
        let context = lunelle_css::StyleComputeContext {
            snapshot_version: 1,
            stylesheet_version: 5,
            environment_version: 9,
            ..lunelle_css::StyleComputeContext::default()
        };
        let (result, diagnostics) = snapshot.compute_with_context(&css, &context);

        assert!(diagnostics.is_empty());
        assert_eq!(result.version, 42);
        assert!(snapshot.accepts_computed_result(&result, 5, 9).is_ok());
        assert!(matches!(
            snapshot.accepts_computed_result(&result, 6, 9),
            Err(AomError::StaleStylesheet {
                expected: 6,
                actual: 5
            })
        ));
        assert!(matches!(
            snapshot.accepts_computed_result(&result, 5, 10),
            Err(AomError::StaleEnvironment {
                expected: 10,
                actual: 9
            })
        ));
    }

    #[test]
    fn async_style_results_preserve_snapshot_version_for_stale_result_rejection() {
        use lunelle_core::TaskPoll;

        let root = Div::new().with_class("theme");
        let old_snapshot = root.style_snapshot(7);
        let current_snapshot = root.style_snapshot(8);
        let css = lunelle_css::CssRender::parse(".theme { color: navy; }");
        let runtime = lunelle_core::Runtime::new(1);
        let task = old_snapshot.compute_async(&runtime, css).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);

        loop {
            match task.try_take() {
                TaskPoll::Ready(Ok((result, diagnostics))) => {
                    assert!(diagnostics.is_empty());
                    assert_eq!(result.version, 7);
                    assert!(current_snapshot.accepts_result(result.version).is_err());
                    break;
                }
                TaskPoll::Ready(Err(error)) => panic!("style computation failed: {error}"),
                TaskPoll::Pending => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "style computation timed out"
                    );
                    std::thread::yield_now();
                }
            }
        }
    }

    #[test]
    fn async_context_results_preserve_all_input_versions() {
        use lunelle_core::TaskPoll;

        let snapshot = Div::new().style_snapshot(17);
        let css = lunelle_css::CssRender::parse("main { color: navy; }");
        let context = lunelle_css::StyleComputeContext {
            stylesheet_version: 23,
            environment_version: 31,
            ..lunelle_css::StyleComputeContext::default()
        };
        let runtime = lunelle_core::Runtime::new(1);
        let task = snapshot
            .compute_async_with_context(&runtime, css, context)
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);

        loop {
            match task.try_take() {
                TaskPoll::Ready(Ok((result, diagnostics))) => {
                    assert!(diagnostics.is_empty());
                    assert_eq!(result.version, 17);
                    assert_eq!(result.stylesheet_version, 23);
                    assert_eq!(result.environment_version, 31);
                    assert!(snapshot.accepts_computed_result(&result, 23, 31).is_ok());
                    break;
                }
                TaskPoll::Ready(Err(error)) => panic!("style computation failed: {error}"),
                TaskPoll::Pending => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "style computation timed out"
                    );
                    std::thread::yield_now();
                }
            }
        }
    }
    #[test]
    fn component_instance_updates_and_releases_lifecycle() {
        let _guard = LIFECYCLE_TEST_LOCK.lock().unwrap();
        LIFECYCLE.store(0, Ordering::Relaxed);
        let mut instance = ComponentInstance::<Counter>::new(2);
        let id = instance.id();
        instance.dispatch(3);
        assert_eq!(instance.element().text(), Some("5"));
        assert_eq!(instance.id(), id);
        drop(instance);
        assert_eq!(LIFECYCLE.load(Ordering::Relaxed), 11);
    }

    struct PanicsInView;
    impl Component for PanicsInView {
        type Props = ();
        type Message = ();
        fn create(_: ()) -> Self {
            Self
        }
        fn update(&mut self, _: ()) {}
        fn view(&self) -> Box<dyn Element> {
            panic!("view failed")
        }
        fn on_unmount(&mut self) {
            PANIC_UNMOUNTS.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn rerender_retains_ids_for_matching_elements() {
        let _guard = LIFECYCLE_TEST_LOCK.lock().unwrap();
        LIFECYCLE.store(0, Ordering::Relaxed);
        let mut instance = ComponentInstance::<Counter>::new(0);
        let root_id = instance.element().id();
        instance.dispatch(1);
        assert_eq!(instance.element().id(), root_id);
    }

    struct KeyedChildren(bool);
    impl Component for KeyedChildren {
        type Props = ();
        type Message = ();
        fn create(_: ()) -> Self {
            Self(false)
        }
        fn update(&mut self, _: ()) {
            self.0 = !self.0;
        }
        fn view(&self) -> Box<dyn Element> {
            let first = Li::new().with_id("first");
            let second = Li::new().with_id("second");
            if self.0 {
                Box::new(List::new().child(second).child(first))
            } else {
                Box::new(List::new().child(first).child(second))
            }
        }
    }

    #[test]
    fn rerender_retains_keyed_child_ids_when_order_changes() {
        let _guard = LIFECYCLE_TEST_LOCK.lock().unwrap();
        let mut instance = ComponentInstance::<KeyedChildren>::new(());
        let first_id = instance.element().children()[0].id();
        let second_id = instance.element().children()[1].id();

        instance.dispatch(());

        assert_eq!(
            instance.element().children()[0].element_id(),
            Some("second")
        );
        assert_eq!(instance.element().children()[0].id(), second_id);
        assert_eq!(instance.element().children()[1].element_id(), Some("first"));
        assert_eq!(instance.element().children()[1].id(), first_id);
    }

    #[test]
    fn failed_initial_view_runs_unmount_hook() {
        PANIC_UNMOUNTS.store(0, Ordering::Relaxed);
        let result = std::panic::catch_unwind(|| ComponentInstance::<PanicsInView>::new(()));
        assert!(result.is_err());
        assert_eq!(PANIC_UNMOUNTS.load(Ordering::Relaxed), 1);
    }
}
