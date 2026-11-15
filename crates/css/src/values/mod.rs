pub use lunelle_core::Color;
use lunelle_macros::css_keyword_enum;

/// CSS length subset used by layout: intrinsic auto, pixels, or percent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    /// Size determined by the layout algorithm.
    Auto,
    /// Absolute pixel length.
    Px(f32),
    /// Length relative to the containing dimension.
    Percent(f32),
}

#[css_keyword_enum(keywords("row" = Row, "column" = Column))]
/// Main axis direction for flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    /// Places items along the horizontal axis.
    Row,
    #[default]
    /// Places items along the vertical axis.
    Column,
}

#[css_keyword_enum(keywords(
    "nowrap" = NoWrap,
    "wrap" = Wrap,
    "wrap-reverse" = WrapReverse,
))]
/// Controls whether flex items remain on one line or wrap onto new lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[css_keyword_enum(keywords(
    "flex-start" = FlexStart,
    "start" = FlexStart,
    "flex-end" = FlexEnd,
    "end" = FlexEnd,
    "center" = Center,
    "space-between" = SpaceBetween,
    "space-around" = SpaceAround,
    "space-evenly" = SpaceEvenly,
))]
/// Distribution of items along a flex or grid axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    /// Packs items at the start of the main axis.
    FlexStart,
    /// Packs items at the end of the main axis.
    FlexEnd,
    /// Centers items on the main axis.
    Center,
    /// Distributes free space between items.
    SpaceBetween,
    /// Distributes free space around items.
    SpaceAround,
    /// Distributes equal free space between and around items.
    SpaceEvenly,
}

#[css_keyword_enum(keywords(
    "flex-start" = FlexStart,
    "start" = FlexStart,
    "flex-end" = FlexEnd,
    "end" = FlexEnd,
    "center" = Center,
    "stretch" = Stretch,
))]
/// Cross-axis alignment for flex or grid items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignItems {
    /// Aligns items at the start of the cross axis.
    FlexStart,
    /// Aligns items at the end of the cross axis.
    FlexEnd,
    /// Centers items on the cross axis.
    Center,
    #[default]
    /// Stretches items across the cross axis.
    Stretch,
}

#[css_keyword_enum(keywords(
    "none" = None,
    "flex" = Flex,
    "block" = Block,
    "inline" = Block,
    "inline-block" = Block,
    "grid" = Grid,
))]
/// Layout algorithm selected by the supported `display` values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    /// Removes the element and its descendants from layout and painting.
    None,
    #[default]
    /// Uses flex layout.
    Flex,
    /// Uses block layout.
    Block,
    /// Uses grid layout.
    Grid,
}

#[css_keyword_enum(keywords(
    "visible" = Visible,
    "hidden" = Hidden,
    "auto" = Auto,
    "scroll" = Scroll,
))]
/// Visibility and scroll behavior for overflowing content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    /// Paints overflowing content.
    Visible,
    /// Clips overflowing content.
    Hidden,
    /// Lets the layout engine choose overflow handling.
    Auto,
    /// Enables scrolling overflow handling.
    Scroll,
}

#[css_keyword_enum(keywords(
    "visible" = Visible,
    "hidden" = Hidden,
    "collapse" = Collapse,
))]
/// Controls whether an element's contents are painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

#[css_keyword_enum(keywords(
    "static" = Static,
    "relative" = Relative,
    "absolute" = Absolute,
))]
/// Supported positioning modes for an element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    /// Keeps the element in normal flow.
    Static,
    /// Offsets the element relative to its normal position.
    Relative,
    /// Positions the element relative to its containing block.
    Absolute,
}

#[css_keyword_enum(keywords(
    "left" = Left,
    "start" = Left,
    "center" = Center,
    "right" = Right,
    "end" = Right,
    "justify" = Justified,
))]
/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    /// Aligns text to the left edge.
    Left,
    /// Centers text.
    Center,
    /// Aligns text to the right edge.
    Right,
    /// Expands spacing so lines meet both edges.
    Justified,
}

/// Line height represented as a font-size multiplier or absolute pixel value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// Multiplies the computed font size.
    Multiplier(f32),
    /// Uses an absolute pixel value.
    Length(f32),
}

impl LineHeight {
    /// Resolves this line-height value using the element's font size.
    pub fn resolve(self, font_size: f32) -> f32 {
        match self {
            Self::Multiplier(multiplier) => multiplier * font_size,
            Self::Length(length) => length,
        }
    }
}

/// Supported grid track sizing forms used by layout conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridTrack {
    /// Automatically sizes a track.
    Auto,
    /// Uses an absolute or percentage track length.
    Length(Length),
    /// Uses a fractional track unit.
    Fraction(f32),
    /// Uses a minimum and maximum track size.
    MinMax(Length, Length),
    /// Uses a length minimum and fractional maximum.
    MinMaxFraction(Length, f32),
    /// Uses a length minimum and automatic maximum.
    MinMaxAuto(Length),
}

#[css_keyword_enum(keywords(
    "row" = Row,
    "column" = Column,
    "row dense" = RowDense,
    "column dense" = ColumnDense,
))]
/// Automatic grid item placement direction and dense packing mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridAutoFlow {
    #[default]
    /// Places items by rows.
    Row,
    /// Places items by columns.
    Column,
    /// Places items by rows using dense packing.
    RowDense,
    /// Places items by columns using dense packing.
    ColumnDense,
}

#[css_keyword_enum(keywords(
    "normal" = Normal,
    "italic" = Italic,
    "oblique" = Oblique,
))]
/// Font slant style supported by the text presentation layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    /// Upright text.
    Normal,
    /// Italic text.
    Italic,
    /// Oblique text.
    Oblique,
}

/// Typed subset of computed CSS consumed by layout and render subsystems.
///
/// Layout fields are converted by the Taffy adapter; box and text fields are
/// consumed by their respective render paths.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Preferred width.
    pub width: Length,
    /// Preferred height.
    pub height: Length,
    /// Minimum width.
    pub min_width: Length,
    /// Maximum width.
    pub max_width: Length,
    /// Minimum height.
    pub min_height: Length,
    /// Maximum height.
    pub max_height: Length,
    /// Selected layout mode.
    pub layout_mode: LayoutMode,
    /// Explicit grid column tracks.
    pub grid_template_columns: Vec<GridTrack>,
    /// Explicit grid row tracks.
    pub grid_template_rows: Vec<GridTrack>,
    /// Automatic column track size.
    pub grid_auto_columns: Option<GridTrack>,
    /// Automatic row track size.
    pub grid_auto_rows: Option<GridTrack>,
    /// Explicit grid column start line.
    pub grid_column_start: Option<i32>,
    /// Explicit grid column end line.
    pub grid_column_end: Option<i32>,
    /// Explicit grid row start line.
    pub grid_row_start: Option<i32>,
    /// Explicit grid row end line.
    pub grid_row_end: Option<i32>,
    /// Named grid column start line.
    pub grid_column_start_name: Option<String>,
    /// Named grid row start line.
    pub grid_row_start_name: Option<String>,
    /// Number of grid columns spanned by an item.
    pub grid_column_span: Option<usize>,
    /// Number of grid rows spanned by an item.
    pub grid_row_span: Option<usize>,
    /// Named grid area assigned to an item.
    pub grid_area: Option<String>,
    /// Automatic item placement policy.
    pub grid_auto_flow: GridAutoFlow,
    /// Flex main-axis direction.
    pub flex_direction: FlexDirection,
    /// Flex line wrapping behavior.
    pub flex_wrap: FlexWrap,
    /// Main-axis content distribution.
    pub justify_content: JustifyContent,
    /// Cross-axis content distribution.
    pub align_content: JustifyContent,
    /// Grid inline-axis item alignment.
    pub justify_items: AlignItems,
    /// Optional grid inline-axis item alignment override.
    pub justify_self: Option<AlignItems>,
    /// Cross-axis item alignment.
    pub align_items: AlignItems,
    /// Optional item-specific cross-axis alignment.
    pub align_self: Option<AlignItems>,
    /// Initial flex basis.
    pub flex_basis: Length,
    /// Flex growth factor.
    pub flex_grow: f32,
    /// Flex shrink factor.
    pub flex_shrink: f32,
    /// Positioning mode.
    pub position: Position,
    /// Offset from the left edge.
    pub left: Length,
    /// Offset from the top edge.
    pub top: Length,
    /// Offset from the right edge.
    pub right: Length,
    /// Offset from the bottom edge.
    pub bottom: Length,
    /// Overflow behavior.
    pub overflow: Overflow,
    /// Horizontal overflow behavior.
    pub overflow_x: Overflow,
    /// Vertical overflow behavior.
    pub overflow_y: Overflow,
    /// Paint visibility of the element and its contents.
    pub visibility: Visibility,
    /// Default gap between layout items.
    pub gap: f32,
    /// Optional row gap.
    pub row_gap: Option<f32>,
    /// Optional column gap.
    pub column_gap: Option<f32>,
    /// Default margin in pixels.
    pub margin: f32,
    /// Top margin override.
    pub margin_top: Option<Length>,
    /// Right margin override.
    pub margin_right: Option<Length>,
    /// Bottom margin override.
    pub margin_bottom: Option<Length>,
    /// Left margin override.
    pub margin_left: Option<Length>,
    /// Default padding in pixels.
    pub padding: f32,
    /// Top padding override.
    pub padding_top: Option<Length>,
    /// Right padding override.
    pub padding_right: Option<Length>,
    /// Bottom padding override.
    pub padding_bottom: Option<Length>,
    /// Left padding override.
    pub padding_left: Option<Length>,
    /// Default border width in pixels.
    pub border_width: f32,
    /// Border color.
    pub border_color: Option<Color>,
    /// Background color.
    pub background: Option<Color>,
    /// Text alignment.
    pub text_align: TextAlign,
    /// Font size in pixels.
    pub font_size: Option<f32>,
    /// Resolved line-height declaration.
    pub line_height: Option<LineHeight>,
    /// Whether text wrapping is enabled.
    pub text_wrap: Option<bool>,
    /// Numeric font weight.
    pub font_weight: Option<u16>,
    /// Font slant style.
    pub font_style: Option<FontStyle>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            max_width: Length::Auto,
            min_height: Length::Auto,
            max_height: Length::Auto,
            layout_mode: LayoutMode::Flex,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_auto_columns: None,
            grid_auto_rows: None,
            grid_column_start: None,
            grid_column_end: None,
            grid_row_start: None,
            grid_row_end: None,
            grid_column_start_name: None,
            grid_row_start_name: None,
            grid_column_span: None,
            grid_row_span: None,
            grid_area: None,
            grid_auto_flow: GridAutoFlow::default(),
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_content: JustifyContent::FlexStart,
            justify_items: AlignItems::Stretch,
            justify_self: None,
            align_items: AlignItems::Stretch,
            align_self: None,
            flex_basis: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            position: Position::Static,
            left: Length::Auto,
            top: Length::Auto,
            right: Length::Auto,
            bottom: Length::Auto,
            overflow: Overflow::Visible,
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            visibility: Visibility::Visible,
            gap: 0.0,
            row_gap: None,
            column_gap: None,
            margin: 0.0,
            margin_top: None,
            margin_right: None,
            margin_bottom: None,
            margin_left: None,
            padding: 0.0,
            padding_top: None,
            padding_right: None,
            padding_bottom: None,
            padding_left: None,
            border_width: 0.0,
            border_color: None,
            background: None,
            text_align: TextAlign::Left,
            font_size: None,
            line_height: None,
            text_wrap: None,
            font_weight: None,
            font_style: None,
        }
    }
}
