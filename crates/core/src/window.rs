use crate::{Point, ScaleFactor, Size};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowOptions {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub decorations: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Application".into(),
            width: 1024,
            height: 768,
            decorations: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    Resized(Size),
    ScaleFactorChanged(ScaleFactor),
    Focused(bool),
    CloseRequested,
    RedrawRequested,
    PointerMoved(Point),
    PointerLeft,
    PointerButton {
        position: Point,
        button: MouseButton,
        pressed: bool,
    },
    Scroll {
        delta: Point,
    },
    Key {
        key: String,
        pressed: bool,
    },
    ModifiersChanged {
        shift: bool,
        control: bool,
        alt: bool,
        super_key: bool,
    },
    Touch {
        id: u64,
        phase: TouchPhase,
        position: Point,
    },
}

/// Platform boundary for creating a window and draining normalized input events.
///
/// Methods are called on the UI/event-loop owner thread. Implementations own platform
/// handles and return owned event values; this trait does not require `Send` or `Sync`.
pub trait WindowBackend {
    type Window;
    type Error;

    /// Creates a window from owned options and returns its platform handle.
    fn create_window(&mut self, options: WindowOptions) -> Result<Self::Window, Self::Error>;

    /// Drains pending events without blocking and transfers ownership of the event list.
    fn poll_events(&mut self) -> Result<Vec<WindowEvent>, Self::Error>;
}
