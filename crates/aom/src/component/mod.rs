use crate::Element;
use crate::event::UiAction;
use lunelle_core::ComponentId;

/// UI-thread-owned component state that produces an owned AOM element tree.
///
/// The caller owns props, messages, and the component instance; `create`, `update`,
/// `view`, and lifecycle hooks run synchronously on that caller's thread. This trait
/// does not require `Send` or `Sync`, and worker tasks must communicate through owned
/// messages rather than access the component instance directly.
pub trait Component {
    type Props;
    type Message;

    /// Constructs component state by taking ownership of its props.
    fn create(props: Self::Props) -> Self
    where
        Self: Sized;

    /// Applies an owned message to this instance on its owning thread.
    fn update(&mut self, message: Self::Message);

    /// Handles a UI action and may update component state synchronously.
    fn action(&mut self, _action: UiAction) {}

    /// Returns an owned element description for reconciliation by the caller.
    fn view(&self) -> Box<dyn Element>;

    /// Runs synchronously after creation and before the first view is retained.
    fn on_mount(&mut self) {}

    /// Runs synchronously when the owning component instance is dropped.
    fn on_unmount(&mut self) {}
}

/// Type-erased owner of a live component instance used by the window runtime.
pub trait ComponentHost {
    fn element(&self) -> &dyn Element;
    fn dispatch_action(&mut self, action: UiAction);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentInstanceId(pub ComponentId);

pub struct ComponentInstance<C: Component> {
    id: ComponentInstanceId,
    component: C,
    element: Box<dyn Element>,
}

impl<C: Component> ComponentInstance<C> {
    pub fn new(props: C::Props) -> Self {
        let mut component = C::create(props);
        let element = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            component.on_mount();
            component.view()
        })) {
            Ok(element) => element,
            Err(payload) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    component.on_unmount()
                }));
                std::panic::resume_unwind(payload);
            }
        };
        Self {
            id: ComponentInstanceId(ComponentId::fresh()),
            component,
            element,
        }
    }

    pub fn id(&self) -> ComponentInstanceId {
        self.id
    }

    pub fn element(&self) -> &dyn Element {
        self.element.as_ref()
    }

    pub fn dispatch(&mut self, message: C::Message) {
        self.component.update(message);
        let mut next = self.component.view();
        crate::diff::retain_matching_ids(self.element.as_ref(), next.as_mut());
        self.element = next;
    }

    /// Dispatches a user action, then rebuilds the component view.
    pub fn dispatch_action(&mut self, action: crate::event::UiAction) {
        self.component.action(action);
        let mut next = self.component.view();
        crate::diff::retain_matching_ids(self.element.as_ref(), next.as_mut());
        self.element = next;
    }
}

impl<C: Component> ComponentHost for ComponentInstance<C> {
    fn element(&self) -> &dyn Element {
        self.element()
    }

    fn dispatch_action(&mut self, action: UiAction) {
        ComponentInstance::dispatch_action(self, action);
    }
}

impl<C: Component> Drop for ComponentInstance<C> {
    fn drop(&mut self) {
        self.component.on_unmount();
    }
}
