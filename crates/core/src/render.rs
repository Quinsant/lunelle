use crate::Color;
use crate::Rect;

/// Backend-neutral draw submission contract; rectangles use logical pixels and implementations own GPU resources.
///
/// Methods are called on the UI/rendering thread. The frame is borrowed only for the
/// duration of `render`; implementations must not retain that borrow. This trait does
/// not require `Send` or `Sync` because rendering resources may be thread-affine.
pub trait Renderer {
    type Error;

    /// Submits a borrowed frame while retaining ownership of backend resources.
    fn render(&mut self, frame: &RenderFrame) -> Result<(), Self::Error>;

    /// Updates the output extent in physical pixels.
    fn resize(&mut self, physical_size: (u32, u32));
}

#[derive(Clone, Debug, Default)]
pub struct RenderFrame {
    pub commands: Vec<DrawCommand>,
}

/// A draw operation expressed in logical pixels, independent of the window's physical DPI scale.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCommand {
    Rect { bounds: Rect, color: [f32; 4] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTargetInfo {
    pub size: crate::Size,
    pub scale_factor: f64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("render target is unavailable")]
    TargetUnavailable,
    #[error("renderer failure: {0}")]
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameDrawCommand {
    Rect {
        rect: Rect,
        clip: Option<Rect>,
        color: Color,
    },
    RoundedRect {
        rect: Rect,
        clip: Option<Rect>,
        color: Color,
        radius: f32,
    },
    Border {
        rect: Rect,
        clip: Option<Rect>,
        color: Color,
        radius: f32,
        thickness: f32,
    },
}

/// Drives frame acquisition, command submission, and presentation on the renderer's owning thread.
///
/// The frame is owned by the caller between `begin_frame` and `end_frame`; implementations
/// should not require `Send` or `Sync` because window and GPU resources may be thread-affine.
pub trait FrameRenderer {
    type Frame;
    type Error;

    /// Acquires a frame for the logical target size and scale factor.
    fn begin_frame(&mut self, target: &RenderTargetInfo) -> Result<Self::Frame, Self::Error>;

    /// Adds backend-neutral commands to the acquired frame.
    fn draw(
        &mut self,
        frame: &mut Self::Frame,
        commands: &[FrameDrawCommand],
    ) -> Result<(), Self::Error>;

    /// Submits and presents the acquired frame.
    fn end_frame(&mut self, frame: Self::Frame) -> Result<(), Self::Error>;
}
