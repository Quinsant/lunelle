use std::{collections::HashMap, sync::Arc};

use crate::Element;
pub use lunelle_core::MouseButton;

/// Component-level actions suitable for state updates.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Click {
        target: Option<String>,
        button: MouseButton,
    },
    Hover {
        target: Option<String>,
        entered: bool,
        x: f32,
        y: f32,
    },
    Input {
        target: Option<String>,
        value: String,
    },
}

/// Event envelope delivered to a stateful component.
///
/// The framework owns the built-in interaction variants. Applications can carry
/// component-specific messages through `Custom(T)` without defining a second
/// event protocol.
#[derive(Clone, Debug, PartialEq)]
pub enum Event<T> {
    Click(UiEvent),
    Hover(UiEvent),
    Input(UiEvent),
    Custom(T),
}

impl<T> Event<T> {
    pub fn from_action(action: UiAction) -> Self {
        match action {
            UiAction::Click { target, button } => Self::Click(UiEvent {
                name: "click".to_owned(),
                target,
                current_target: None,
                phase: EventPhase::Target,
                key: None,
                value: None,
                mouse_button: Some(button),
                x: 0.0,
                y: 0.0,
                delta_x: 0.0,
                delta_y: 0.0,
                propagation_stopped: false,
            }),
            UiAction::Hover {
                target,
                entered,
                x,
                y,
            } => Self::Hover(UiEvent {
                name: if entered { "mouseenter" } else { "mouseleave" }.to_owned(),
                target,
                current_target: None,
                phase: EventPhase::Target,
                key: None,
                value: None,
                mouse_button: None,
                x,
                y,
                delta_x: 0.0,
                delta_y: 0.0,
                propagation_stopped: false,
            }),
            UiAction::Input { target, value } => Self::Input(UiEvent {
                name: "input".to_owned(),
                target,
                current_target: None,
                phase: EventPhase::Target,
                key: None,
                value: Some(value),
                mouse_button: None,
                x: 0.0,
                y: 0.0,
                delta_x: 0.0,
                delta_y: 0.0,
                propagation_stopped: false,
            }),
        }
    }

    pub fn custom(value: T) -> Self {
        Self::Custom(value)
    }

    pub fn click(event: UiEvent) -> Self {
        Self::Click(event)
    }

    pub fn hover(event: UiEvent) -> Self {
        Self::Hover(event)
    }

    pub fn input(event: UiEvent) -> Self {
        Self::Input(event)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiEvent {
    pub name: String,
    pub target: Option<String>,
    pub current_target: Option<String>,
    pub phase: EventPhase,
    pub key: Option<String>,
    pub value: Option<String>,
    pub mouse_button: Option<MouseButton>,
    pub x: f32,
    pub y: f32,
    pub delta_x: f32,
    pub delta_y: f32,
    propagation_stopped: bool,
}

impl UiEvent {
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }
}

/// Read-only view of an event tree borrowed for synchronous UI-thread dispatch.
///
/// Implementations are not required to be `Send` or `Sync`; dispatch borrows the tree
/// and its children only for the duration of the event traversal.
pub trait EventTree {}

type EventHandler = Arc<dyn Fn(&mut UiEvent) + Send + Sync>;

#[derive(Default)]
pub struct EventDispatcher {
    handlers: HashMap<(String, String, EventPhase), Vec<EventHandler>>,
}

impl EventDispatcher {
    pub fn on(
        &mut self,
        id: impl Into<String>,
        event_name: impl Into<String>,
        handler: impl Fn(&mut UiEvent) + Send + Sync + 'static,
    ) {
        self.handlers
            .entry((id.into(), event_name.into(), EventPhase::Bubble))
            .or_default()
            .push(Arc::new(handler));
    }

    pub fn on_phase(
        &mut self,
        id: impl Into<String>,
        event_name: impl Into<String>,
        phase: EventPhase,
        handler: impl Fn(&mut UiEvent) + Send + Sync + 'static,
    ) {
        self.handlers
            .entry((id.into(), event_name.into(), phase))
            .or_default()
            .push(Arc::new(handler));
    }

    pub fn on_capture(
        &mut self,
        id: impl Into<String>,
        event_name: impl Into<String>,
        handler: impl Fn(&mut UiEvent) + Send + Sync + 'static,
    ) {
        self.on_phase(id, event_name, EventPhase::Capture, handler);
    }

    pub fn dispatch(&self, root: &dyn Element, target_id: &str, event_name: &str) -> bool {
        self.dispatch_event(root, target_id, event_name, None, None, 0.0, 0.0, 0.0, 0.0)
    }

    pub fn dispatch_pointer(
        &self,
        root: &dyn Element,
        target_id: &str,
        event_name: &str,
        x: f32,
        y: f32,
    ) -> bool {
        self.dispatch_event(root, target_id, event_name, None, None, x, y, 0.0, 0.0)
    }

    pub fn dispatch_mouse(
        &self,
        root: &dyn Element,
        target_id: &str,
        event_name: &str,
        button: MouseButton,
    ) -> bool {
        self.dispatch_event(
            root,
            target_id,
            event_name,
            None,
            Some(button),
            0.0,
            0.0,
            0.0,
            0.0,
        )
    }

    pub fn dispatch_scroll(
        &self,
        root: &dyn Element,
        target_id: &str,
        delta_x: f32,
        delta_y: f32,
    ) -> bool {
        self.dispatch_event(
            root,
            target_id,
            "scroll",
            None,
            None,
            0.0,
            0.0,
            if delta_x.is_finite() { delta_x } else { 0.0 },
            if delta_y.is_finite() { delta_y } else { 0.0 },
        )
    }

    pub fn dispatch_keyboard(
        &self,
        root: &dyn Element,
        target_id: &str,
        event_name: &str,
        key: &str,
    ) -> bool {
        self.dispatch_event(
            root,
            target_id,
            event_name,
            Some(key.to_owned()),
            None,
            0.0,
            0.0,
            0.0,
            0.0,
        )
    }

    fn dispatch_event(
        &self,
        root: &dyn Element,
        target_id: &str,
        event_name: &str,
        key: Option<String>,
        mouse_button: Option<MouseButton>,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    ) -> bool {
        let mut path = Vec::new();
        if !find_path(root, target_id, &mut path) {
            return false;
        }
        let mut event = UiEvent {
            name: event_name.to_owned(),
            target: Some(target_id.to_owned()),
            current_target: None,
            phase: EventPhase::Capture,
            key,
            value: None,
            mouse_button,
            x,
            y,
            delta_x,
            delta_y,
            propagation_stopped: false,
        };

        for id in path.iter().take(path.len().saturating_sub(1)) {
            event.current_target = Some(id.clone());
            if let Some(handlers) =
                self.handlers
                    .get(&(id.clone(), event_name.to_owned(), EventPhase::Capture))
            {
                for handler in handlers {
                    handler(&mut event);
                    if event.propagation_stopped {
                        return true;
                    }
                }
            }
        }

        event.phase = EventPhase::Target;
        event.current_target = Some(target_id.to_owned());
        for phase in [EventPhase::Capture, EventPhase::Bubble] {
            if let Some(handlers) =
                self.handlers
                    .get(&(target_id.to_owned(), event_name.to_owned(), phase))
            {
                for handler in handlers {
                    handler(&mut event);
                    if event.propagation_stopped {
                        return true;
                    }
                }
            }
        }

        event.phase = EventPhase::Bubble;
        for id in path.iter().rev().skip(1) {
            event.current_target = Some(id.clone());
            if let Some(handlers) =
                self.handlers
                    .get(&(id.clone(), event_name.to_owned(), EventPhase::Bubble))
            {
                for handler in handlers {
                    handler(&mut event);
                    if event.propagation_stopped {
                        return true;
                    }
                }
            }
        }
        true
    }
}

pub fn dispatch_pointer_events(
    events: &EventDispatcher,
    root: Option<&dyn Element>,
    pointer_events: Vec<(String, String)>,
) {
    if let Some(root) = root {
        for (target, name) in pointer_events {
            events.dispatch(root, &target, &name);
        }
    }
}

pub fn dispatch_drag_cancel(
    events: &EventDispatcher,
    root: Option<&dyn Element>,
    drag: (bool, Option<String>, Option<String>),
) {
    let (was_dragging, source, target) = drag;
    let Some(root) = root else {
        return;
    };
    if was_dragging {
        if let Some(target) = target {
            events.dispatch(root, &target, "dragleave");
        }
        if let Some(source) = source {
            events.dispatch(root, &source, "dragend");
        }
    }
}

pub fn dispatch_mouse_release(
    events: &EventDispatcher,
    root: &dyn Element,
    pressed_id: Option<&str>,
    released_id: Option<&str>,
    was_dragging: bool,
    button: MouseButton,
) -> bool {
    if let Some(released_id) = released_id {
        events.dispatch_mouse(root, released_id, "mouseup", button);
    }
    let clicked = !was_dragging && pressed_id.is_some() && pressed_id == released_id;
    if clicked && let Some(target) = released_id {
        events.dispatch(root, target, "click");
    }
    clicked
}

fn find_path(element: &dyn Element, target_id: &str, path: &mut Vec<String>) -> bool {
    if let Some(id) = element.element_id() {
        path.push(id.to_owned());
        if id == target_id {
            return true;
        }
    }
    for child in element.children() {
        if find_path(child.as_ref(), target_id, path) {
            return true;
        }
    }
    if element.element_id().is_some() {
        path.pop();
    }
    false
}
