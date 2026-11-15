//! Small parsing and normalization helpers shared by CSS modules.
//!
//! Public helpers are re-exported by the crate facade; module-conversion
//! helpers remain crate-private because they describe the internal `Style`
//! projection rather than the public CSS syntax API.
mod color;
mod length;
mod modules;
mod parser;
mod properties;

pub use color::{parse_color, style_opacity};
pub use length::{css_length, rem_to_px};
pub(crate) use modules::{
    align_items, css_number, edge_value, grid_auto_flow, grid_track, grid_tracks, integer,
    justify_content, length, length_number, length_string, number, placement, span,
};
pub use parser::{split_top_level, split_top_level_once, split_top_level_whitespace};
pub use properties::is_known_property;
