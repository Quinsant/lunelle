//! Thin application composition facade over the framework runtime crates.

pub use lunelle_aom as aom;
pub use lunelle_core as core;
pub use lunelle_css as css;
pub use lunelle_macros as macros;
pub use lunelle_macros::{component, view};

mod app;
pub use app::{
    Application, ApplicationBuilder, ApplicationContext, ApplicationMessage, WindowSpawnError,
};

mod windowed;
pub use windowed::RunError;
