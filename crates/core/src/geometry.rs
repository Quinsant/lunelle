use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);
        impl $name {
            pub fn fresh() -> Self {
                Self(next_id())
            }
            pub const fn from_raw(value: u64) -> Self {
                Self(value)
            }
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(ElementId);
id_type!(ComponentId);
id_type!(WindowId);
id_type!(NodeId);

/// A position in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
/// A size in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
/// A rectangle in logical pixels, with its origin measured from the top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
/// The number of physical pixels per logical pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleFactor(f64);

impl ScaleFactor {
    /// Creates a scale factor if it and its reciprocal are finite and representable as `f32`.
    pub fn new(value: f64) -> Option<Self> {
        let as_f32 = value as f32;
        (value.is_finite()
            && value > 0.0
            && as_f32.is_finite()
            && as_f32 > 0.0
            && (1.0 / as_f32).is_finite())
        .then_some(Self(value))
    }
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Size {
    /// Converts physical pixels to logical pixels. Invalid scale factors are treated as `1.0`.
    pub fn from_physical(width: u32, height: u32, scale_factor: f64) -> Self {
        let scale_factor = ScaleFactor::new(scale_factor).map_or(1.0, |scale| scale.get() as f32);
        Self {
            width: width as f32 / scale_factor,
            height: height as f32 / scale_factor,
        }
    }
}
