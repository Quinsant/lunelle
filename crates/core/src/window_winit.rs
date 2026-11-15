use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use thiserror::Error;
#[cfg(target_os = "linux")]
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent as WinitWindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    window::{Window, WindowAttributes, WindowId as WinitWindowId},
};

use crate::{
    MouseButton, ScaleFactor, Size, TouchPhase, WindowBackend, WindowEvent, WindowOptions,
};

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("failed to initialize winit event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[error("failed to create winit window: {0}")]
    CreateWindow(#[from] winit::error::OsError),
    #[error("winit event loop exited while polling")]
    Exited,
    #[error("window has not been created yet")]
    NotCreated,
}

/// Winit window adapter. Call all methods on the event-loop owner thread.
pub struct WinitBackend {
    event_loop: Option<EventLoop<()>>,
    options: Option<WindowOptions>,
    window: Option<Arc<Window>>,
    window_id: Option<WinitWindowId>,
    events: VecDeque<WindowEvent>,
    normalizer: EventNormalizer,
    close_requested: bool,
    finished: bool,
    error: Option<WindowError>,
}

impl WinitBackend {
    pub fn new() -> Result<Self, WindowError> {
        Ok(Self {
            event_loop: Some(EventLoop::new()?),
            options: None,
            window: None,
            window_id: None,
            events: VecDeque::new(),
            normalizer: EventNormalizer::default(),
            close_requested: false,
            finished: false,
            error: None,
        })
    }

    pub fn window(&self) -> Option<&Arc<Window>> {
        self.window.as_ref()
    }
}

impl ApplicationHandler for WinitBackend {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let Some(options) = self.options.take() else {
            return;
        };
        let attributes: WindowAttributes = Window::default_attributes()
            .with_title(options.title)
            .with_inner_size(winit::dpi::LogicalSize::new(options.width, options.height))
            .with_decorations(options.decorations)
            .with_name("lunelle", "Lunelle");
        match event_loop.create_window(attributes) {
            Ok(window) => {
                self.normalizer.scale_factor = window.scale_factor();
                self.window_id = Some(window.id());
                self.window = Some(Arc::new(window));
            }
            Err(error) => {
                self.close_requested = true;
                self.error = Some(WindowError::CreateWindow(error));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WinitWindowId,
        event: WinitWindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }
        if matches!(event, WinitWindowEvent::CloseRequested) {
            self.close_requested = true;
            self.events.push_back(WindowEvent::CloseRequested);
            event_loop.exit();
            return;
        }
        if let Some(event) = self.normalizer.normalize(event) {
            self.events.push_back(event);
        }
    }
}

impl WindowBackend for WinitBackend {
    type Window = Arc<Window>;
    type Error = WindowError;

    fn create_window(&mut self, options: WindowOptions) -> Result<Self::Window, Self::Error> {
        if self.finished {
            return Err(WindowError::Exited);
        }
        self.options = Some(options);
        self.close_requested = false;
        let mut event_loop = self.event_loop.take().ok_or(WindowError::Exited)?;
        let status = event_loop.pump_app_events(Some(Duration::ZERO), self);
        self.event_loop = Some(event_loop);
        if matches!(status, PumpStatus::Exit(_)) {
            self.finished = true;
        }
        if self.finished || self.close_requested && self.window.is_none() {
            return Err(self.error.take().unwrap_or(WindowError::Exited));
        }
        self.window.clone().ok_or(WindowError::NotCreated)
    }

    fn poll_events(&mut self) -> Result<Vec<WindowEvent>, Self::Error> {
        if self.finished && self.close_requested {
            return Ok(self.events.drain(..).collect());
        }
        if self.finished {
            return Err(WindowError::Exited);
        }
        let mut event_loop = self.event_loop.take().ok_or(WindowError::Exited)?;
        let status = event_loop.pump_app_events(Some(Duration::ZERO), self);
        self.event_loop = Some(event_loop);
        if matches!(status, PumpStatus::Exit(_)) {
            self.finished = true;
            if !self.close_requested {
                return Err(WindowError::Exited);
            }
        }
        Ok(self.events.drain(..).collect())
    }
}

/// Callback-driven application lifecycle hosted by the core winit event loop.
/// Callbacks run on the event-loop thread and receive owned normalized values; return
/// `true` to exit the application. The application remains owned by the host and is not
/// required to be `Send` or `Sync` because winit invokes it on that thread.
pub trait WindowApplication {
    /// Receives ownership of the newly created window handle.
    fn resumed(&mut self, window: Arc<Window>) -> bool;

    /// Receives an owned backend-neutral event.
    fn window_event(&mut self, event: WindowEvent) -> bool;

    /// Advances time-based application work and selects the event-loop action.
    fn about_to_wait(&mut self) -> ApplicationControl {
        ApplicationControl::Wait
    }

    /// Releases resources that depend on the event loop before it is destroyed.
    fn exiting(&mut self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControl {
    Exit,
    Wait,
    Redraw,
    Poll,
}

struct ApplicationHost<A> {
    application: A,
    title: String,
    width: u32,
    height: u32,
    decorations: bool,
    window_id: Option<WinitWindowId>,
    window: Option<Arc<Window>>,
    normalizer: EventNormalizer,
    error: Option<WindowError>,
}

/// Runs an application using winit's callback-driven event loop and returns its state on exit.
pub fn run_application<A: WindowApplication + 'static>(
    options: WindowOptions,
    application: A,
) -> Result<A, WindowError> {
    let event_loop = EventLoop::new()?;
    let mut host = ApplicationHost {
        application,
        title: options.title,
        width: options.width,
        height: options.height,
        decorations: options.decorations,
        window_id: None,
        window: None,
        normalizer: EventNormalizer::default(),
        error: None,
    };
    event_loop.run_app(&mut host)?;
    if let Some(error) = host.error {
        return Err(error);
    }
    Ok(host.application)
}

impl<A: WindowApplication> ApplicationHandler for ApplicationHost<A> {
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.application.exiting();
        self.window = None;
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_id.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height))
            .with_decorations(self.decorations)
            .with_name("lunelle", "Lunelle");
        match event_loop.create_window(attributes) {
            Ok(window) => {
                self.normalizer.scale_factor = window.scale_factor();
                self.window_id = Some(window.id());
                let window = Arc::new(window);
                self.window = Some(window.clone());
                if self.application.resumed(window) {
                    event_loop.exit();
                }
            }
            Err(error) => {
                self.error = Some(WindowError::CreateWindow(error));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WinitWindowId,
        event: WinitWindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }
        let event = if matches!(event, WinitWindowEvent::CloseRequested) {
            Some(WindowEvent::CloseRequested)
        } else {
            self.normalizer.normalize(event)
        };
        if event.is_some_and(|event| self.application.window_event(event)) {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.application.about_to_wait() {
            ApplicationControl::Exit => event_loop.exit(),
            ApplicationControl::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            ApplicationControl::Poll => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
            ApplicationControl::Redraw => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            }
        }
    }
}

struct EventNormalizer {
    scale_factor: f64,
    pointer: crate::Point,
}

impl Default for EventNormalizer {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            pointer: crate::Point::default(),
        }
    }
}

impl EventNormalizer {
    fn set_scale_factor(&mut self, scale_factor: f64) -> Option<ScaleFactor> {
        let scale_factor = ScaleFactor::new(scale_factor)?;
        self.scale_factor = scale_factor.get();
        Some(scale_factor)
    }

    fn normalize(&mut self, event: WinitWindowEvent) -> Option<WindowEvent> {
        match event {
            WinitWindowEvent::RedrawRequested => Some(WindowEvent::RedrawRequested),
            WinitWindowEvent::Resized(size) => Some(WindowEvent::Resized(Size::from_physical(
                size.width,
                size.height,
                self.scale_factor,
            ))),
            WinitWindowEvent::ScaleFactorChanged { scale_factor, .. } => self
                .set_scale_factor(scale_factor)
                .map(WindowEvent::ScaleFactorChanged),
            WinitWindowEvent::Focused(focused) => Some(WindowEvent::Focused(focused)),
            WinitWindowEvent::Occluded(false) => Some(WindowEvent::RedrawRequested),
            WinitWindowEvent::CursorMoved { position, .. } => {
                self.pointer = physical_to_logical(position, self.scale_factor);
                Some(WindowEvent::PointerMoved(self.pointer))
            }
            WinitWindowEvent::CursorLeft { .. } => Some(WindowEvent::PointerLeft),
            WinitWindowEvent::MouseInput { state, button, .. } => {
                Some(WindowEvent::PointerButton {
                    position: self.pointer,
                    button: normalize_mouse_button(button),
                    pressed: state == ElementState::Pressed,
                })
            }
            WinitWindowEvent::MouseWheel { delta, .. } => Some(WindowEvent::Scroll {
                delta: normalize_scroll_delta(delta, self.scale_factor),
            }),
            WinitWindowEvent::KeyboardInput { event, .. } => Some(WindowEvent::Key {
                key: format!("{:?}", event.logical_key),
                pressed: event.state == ElementState::Pressed,
            }),
            WinitWindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                Some(WindowEvent::ModifiersChanged {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    super_key: state.super_key(),
                })
            }
            WinitWindowEvent::Touch(touch) => Some(WindowEvent::Touch {
                id: touch.id,
                phase: match touch.phase {
                    winit::event::TouchPhase::Started => TouchPhase::Started,
                    winit::event::TouchPhase::Moved => TouchPhase::Moved,
                    winit::event::TouchPhase::Ended => TouchPhase::Ended,
                    winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
                },
                position: physical_to_logical(touch.location, self.scale_factor),
            }),
            _ => None,
        }
    }
}

pub fn physical_to_logical(
    position: winit::dpi::PhysicalPosition<f64>,
    scale_factor: f64,
) -> crate::Point {
    let scale = ScaleFactor::new(scale_factor).map_or(1.0, ScaleFactor::get) as f32;
    crate::Point {
        x: position.x as f32 / scale,
        y: position.y as f32 / scale,
    }
}

pub fn normalize_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Back,
        winit::event::MouseButton::Forward => MouseButton::Forward,
        winit::event::MouseButton::Other(button) => MouseButton::Other(button),
    }
}

pub fn normalize_scroll_delta(
    delta: winit::event::MouseScrollDelta,
    scale_factor: f64,
) -> crate::Point {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => crate::Point {
            x: x * 40.0,
            y: y * 40.0,
        },
        winit::event::MouseScrollDelta::PixelDelta(position) => {
            physical_to_logical(position, scale_factor)
        }
    }
}

#[derive(Default)]
pub struct TouchTracker {
    active: Option<(u64, f32, f32, f32, f32, bool)>,
}

#[derive(Debug, PartialEq)]
pub enum TouchUpdate {
    Started((f32, f32)),
    Moved {
        position: (f32, f32),
        delta: (f32, f32),
        scroll_delta: (f32, f32),
        scroll_started: bool,
    },
    Ended {
        position: (f32, f32),
        tap: bool,
    },
    Cancelled,
}

impl TouchTracker {
    pub fn update(
        &mut self,
        id: u64,
        phase: winit::event::TouchPhase,
        position: winit::dpi::PhysicalPosition<f64>,
        scale_factor: f64,
    ) -> Option<TouchUpdate> {
        let point = physical_to_logical(position, scale_factor);
        let (x, y) = (point.x, point.y);

        match phase {
            winit::event::TouchPhase::Started if self.active.is_none() => {
                self.active = Some((id, x, y, x, y, true));
                Some(TouchUpdate::Started((x, y)))
            }
            winit::event::TouchPhase::Moved => {
                if let Some((active_id, start_x, start_y, last_x, last_y, tap_eligible)) =
                    self.active
                    && active_id == id
                {
                    let tap_eligible =
                        tap_eligible && (x - start_x).powi(2) + (y - start_y).powi(2) <= 64.0;
                    self.active = Some((active_id, start_x, start_y, x, y, tap_eligible));
                    return Some(TouchUpdate::Moved {
                        position: (x, y),
                        delta: (x - last_x, y - last_y),
                        scroll_delta: (x - start_x, y - start_y),
                        scroll_started: !tap_eligible,
                    });
                }
                None
            }
            winit::event::TouchPhase::Ended => {
                if let Some((active_id, start_x, start_y, _, _, tap_eligible)) = self.active
                    && active_id == id
                {
                    self.active = None;
                    return Some(TouchUpdate::Ended {
                        position: (x, y),
                        tap: tap_eligible && (x - start_x).powi(2) + (y - start_y).powi(2) <= 64.0,
                    });
                }
                None
            }
            winit::event::TouchPhase::Cancelled => {
                if self
                    .active
                    .is_some_and(|(active_id, _, _, _, _, _)| active_id == id)
                {
                    self.active = None;
                    return Some(TouchUpdate::Cancelled);
                }
                None
            }
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct MouseDragTracker {
    source: Option<String>,
    origin: Option<(f32, f32)>,
    current_target: Option<String>,
    dragging: bool,
}

impl MouseDragTracker {
    pub fn begin(&mut self, source: String, position: (f32, f32)) {
        self.source = Some(source.clone());
        self.origin = Some(position);
        self.current_target = Some(source);
        self.dragging = false;
    }

    pub fn update(&mut self, position: (f32, f32), target: Option<&str>) -> Vec<(String, String)> {
        let (Some(source), Some(origin)) = (self.source.clone(), self.origin) else {
            return Vec::new();
        };
        if !self.dragging
            && (position.0 - origin.0).powi(2) + (position.1 - origin.1).powi(2) > 64.0
        {
            self.dragging = true;
            let mut events = vec![(source.clone(), "dragstart".to_owned())];
            self.update_target(target, &mut events);
            events.push((source, "drag".to_owned()));
            return events;
        }
        if !self.dragging {
            return Vec::new();
        }
        let mut events = Vec::new();
        self.update_target(target, &mut events);
        events.push((source, "drag".to_owned()));
        events
    }

    fn update_target(&mut self, target: Option<&str>, events: &mut Vec<(String, String)>) {
        if self.current_target.as_deref() == target {
            return;
        }
        if let Some(previous) = self.current_target.take() {
            events.push((previous, "dragleave".to_owned()));
        }
        if let Some(target) = target {
            self.current_target = Some(target.to_owned());
            events.push((target.to_owned(), "dragenter".to_owned()));
        }
    }

    pub fn finish(&mut self) -> (bool, Option<String>, Option<String>) {
        let was_dragging = self.dragging;
        let source = self.source.take();
        let target = self.current_target.take();
        self.origin = None;
        self.dragging = false;
        (was_dragging, source, target)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventNormalizer, WinitWindowEvent, normalize_mouse_button, normalize_scroll_delta,
        physical_to_logical,
    };
    use crate::{MouseButton, Point, Size, WindowEvent};
    use winit::{
        dpi::PhysicalPosition,
        event::{MouseButton as WinitMouseButton, MouseScrollDelta},
    };

    #[test]
    fn physical_pointer_coordinates_are_converted_to_logical_pixels() {
        let point = physical_to_logical(PhysicalPosition::new(120.0, 60.0), 2.0);
        assert_eq!(point, Point { x: 60.0, y: 30.0 });
    }

    #[test]
    fn mouse_buttons_are_normalized_without_losing_other_button_id() {
        assert_eq!(
            normalize_mouse_button(WinitMouseButton::Left),
            MouseButton::Left
        );
        assert_eq!(
            normalize_mouse_button(WinitMouseButton::Other(7)),
            MouseButton::Other(7)
        );
    }

    #[test]
    fn scroll_deltas_use_logical_pixels() {
        assert_eq!(
            normalize_scroll_delta(MouseScrollDelta::LineDelta(1.0, -2.0), 1.0),
            Point { x: 40.0, y: -80.0 }
        );
        assert_eq!(
            normalize_scroll_delta(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(80.0, 40.0)),
                2.0
            ),
            Point { x: 40.0, y: 20.0 }
        );
    }

    #[test]
    fn updated_scale_factor_is_used_for_resize_and_pointer_coordinates() {
        let mut normalizer = EventNormalizer::default();
        assert_eq!(normalizer.scale_factor, 1.0);
        assert_eq!(normalizer.set_scale_factor(2.0).unwrap().get(), 2.0);

        assert_eq!(
            normalizer.normalize(WinitWindowEvent::Resized(winit::dpi::PhysicalSize::new(
                800, 600
            ))),
            Some(WindowEvent::Resized(Size {
                width: 400.0,
                height: 300.0
            })),
        );
        assert_eq!(
            normalizer.normalize(WinitWindowEvent::CursorMoved {
                device_id: winit::event::DeviceId::dummy(),
                position: PhysicalPosition::new(120.0, 60.0),
            }),
            Some(WindowEvent::PointerMoved(Point { x: 60.0, y: 30.0 })),
        );
    }

    #[test]
    fn pointer_leave_and_modifier_state_are_normalized() {
        let mut normalizer = EventNormalizer::default();
        assert_eq!(
            normalizer.normalize(WinitWindowEvent::CursorLeft {
                device_id: winit::event::DeviceId::dummy(),
            }),
            Some(WindowEvent::PointerLeft),
        );

        let modifiers = winit::keyboard::ModifiersState::SHIFT
            | winit::keyboard::ModifiersState::CONTROL
            | winit::keyboard::ModifiersState::ALT;
        assert_eq!(
            normalizer.normalize(WinitWindowEvent::ModifiersChanged(
                winit::event::Modifiers::from(modifiers),
            )),
            Some(WindowEvent::ModifiersChanged {
                shift: true,
                control: true,
                alt: true,
                super_key: false,
            }),
        );
    }

    #[test]
    fn touch_events_are_normalized_to_logical_coordinates() {
        let mut normalizer = EventNormalizer::default();
        normalizer.set_scale_factor(2.0).unwrap();
        assert_eq!(
            normalizer.normalize(WinitWindowEvent::Touch(winit::event::Touch {
                device_id: winit::event::DeviceId::dummy(),
                id: 7,
                phase: winit::event::TouchPhase::Started,
                location: PhysicalPosition::new(120.0, 60.0),
                force: None,
            })),
            Some(WindowEvent::Touch {
                id: 7,
                phase: crate::TouchPhase::Started,
                position: Point { x: 60.0, y: 30.0 },
            }),
        );
    }
}
