use taffy::{
    Dimension, Display, FlexDirection, FlexWrap, GridTemplateComponent, LengthPercentage, Style,
    prelude::{
        AlignContent, AlignItems, FromFr, FromLength, FromPercent, JustifyContent, TaffyAuto, fr,
        line, minmax, span,
    },
};

use crate::{ComputedStyle, CssComputedState, GridTrack, LayoutMode, Length, Overflow};

/// Layout side effects that Taffy style values alone cannot represent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CssLayoutStyle {
    /// Whether the element supports scrolling overflow.
    pub scrollable: bool,
    /// Whether children are clipped to the element bounds.
    pub clips_children: bool,
    /// Whether width was explicitly declared.
    pub explicit_width: bool,
    /// Whether minimum width was explicitly declared.
    pub explicit_min_width: bool,
    /// Whether height was explicitly declared.
    pub explicit_height: bool,
}

/// Converts serialized computed CSS properties to Taffy's layout style.
pub fn taffy_style(computed: &ComputedStyle) -> (Style, CssLayoutStyle) {
    taffy_style_state(&CssComputedState::from_computed(computed))
}

/// Converts an already projected CSS state to Taffy layout inputs.
pub fn taffy_style_state(state: &CssComputedState) -> (Style, CssLayoutStyle) {
    let values = &state.values;
    let mut style = Style::default();
    style.display = match values.layout_mode {
        LayoutMode::None => Display::None,
        LayoutMode::Grid => Display::Grid,
        LayoutMode::Block => Display::Block,
        LayoutMode::Flex => Display::Flex,
    };
    style.flex_direction = match values.flex_direction {
        crate::FlexDirection::Row => FlexDirection::Row,
        crate::FlexDirection::Column => FlexDirection::Column,
    };
    style.flex_wrap = match values.flex_wrap {
        crate::FlexWrap::NoWrap => FlexWrap::NoWrap,
        crate::FlexWrap::Wrap => FlexWrap::Wrap,
        crate::FlexWrap::WrapReverse => FlexWrap::WrapReverse,
    };
    style.justify_content = match values.justify_content {
        crate::JustifyContent::Center => JustifyContent::CENTER,
        crate::JustifyContent::FlexEnd => JustifyContent::END,
        crate::JustifyContent::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        crate::JustifyContent::SpaceAround => JustifyContent::SPACE_AROUND,
        crate::JustifyContent::SpaceEvenly => JustifyContent::SPACE_EVENLY,
        crate::JustifyContent::FlexStart => JustifyContent::FLEX_START,
    };
    style.align_content = match values.align_content {
        crate::JustifyContent::Center => AlignContent::CENTER,
        crate::JustifyContent::FlexEnd => AlignContent::FLEX_END,
        crate::JustifyContent::SpaceBetween => AlignContent::SPACE_BETWEEN,
        crate::JustifyContent::SpaceAround => AlignContent::SPACE_AROUND,
        crate::JustifyContent::SpaceEvenly => AlignContent::SPACE_EVENLY,
        crate::JustifyContent::FlexStart => AlignContent::FLEX_START,
    };
    style.justify_items = typed_align_items(values.justify_items);
    style.justify_self = values.justify_self.map(typed_align_self);
    style.align_items = match values.align_items {
        crate::AlignItems::Center => AlignItems::CENTER,
        crate::AlignItems::FlexStart => AlignItems::FLEX_START,
        crate::AlignItems::FlexEnd => AlignItems::FLEX_END,
        crate::AlignItems::Stretch => AlignItems::STRETCH,
    };
    style.position = match values.position {
        crate::Position::Static => taffy::Position::Static,
        crate::Position::Relative => taffy::Position::Relative,
        crate::Position::Absolute => taffy::Position::Absolute,
    };
    style.inset = typed_edge_rect_auto([
        Some(values.top),
        Some(values.right),
        Some(values.bottom),
        Some(values.left),
    ]);
    style.align_self = values.align_self.map(typed_align_self);
    style.gap.width = values
        .column_gap
        .map(LengthPercentage::length)
        .unwrap_or_else(|| LengthPercentage::length(values.gap));
    style.gap.height = values
        .row_gap
        .map(LengthPercentage::length)
        .unwrap_or_else(|| LengthPercentage::length(values.gap));
    style.padding = typed_edge_rect([
        values.padding_top,
        values.padding_right,
        values.padding_bottom,
        values.padding_left,
    ]);
    style.margin = typed_edge_rect_auto([
        values.margin_top,
        values.margin_right,
        values.margin_bottom,
        values.margin_left,
    ]);
    style.border = typed_edge_rect([Some(Length::Px(values.border_width)); 4]);
    style.size.width = typed_dimension(values.width);
    style.size.height = typed_dimension(values.height);
    style.min_size.width = typed_dimension_auto(values.min_width);
    style.min_size.height = typed_dimension_auto(values.min_height);
    style.max_size.width = typed_dimension_auto(values.max_width);
    style.max_size.height = typed_dimension_auto(values.max_height);
    style.flex_grow = values.flex_grow;
    style.flex_shrink = values.flex_shrink;
    style.flex_basis = typed_dimension(values.flex_basis);
    style.overflow = taffy::Point {
        x: typed_overflow(values.overflow_x),
        y: typed_overflow(values.overflow_y),
    };
    style.grid_template_columns = typed_grid_tracks(&values.grid_template_columns);
    style.grid_template_rows = typed_grid_tracks(&values.grid_template_rows);
    if let Some(start) = values.grid_column_start {
        style.grid_column.start = line(start as i16);
    }
    if let Some(end) = values.grid_column_end {
        style.grid_column.end = line(end as i16);
    }
    if values.grid_column_end.is_none() {
        if let Some(span_value) = values.grid_column_span {
            style.grid_column.end = span(span_value as u16);
        }
    }
    if let Some(start) = values.grid_row_start {
        style.grid_row.start = line(start as i16);
    }
    if let Some(end) = values.grid_row_end {
        style.grid_row.end = line(end as i16);
    }
    if values.grid_row_end.is_none() {
        if let Some(span_value) = values.grid_row_span {
            style.grid_row.end = span(span_value as u16);
        }
    }
    (
        style,
        CssLayoutStyle {
            scrollable: matches!(values.overflow_x, Overflow::Auto | Overflow::Scroll)
                || matches!(values.overflow_y, Overflow::Auto | Overflow::Scroll),
            clips_children: !matches!(values.overflow_x, Overflow::Visible)
                || !matches!(values.overflow_y, Overflow::Visible),
            explicit_width: !matches!(values.width, Length::Auto),
            explicit_min_width: !matches!(values.min_width, Length::Auto),
            explicit_height: !matches!(values.height, Length::Auto),
        },
    )
}

fn typed_length(value: Length) -> Option<LengthPercentage> {
    match value {
        Length::Auto => None,
        Length::Px(value) => Some(LengthPercentage::length(value)),
        Length::Percent(value) => Some(LengthPercentage::percent(value / 100.0)),
    }
}

fn typed_align_items(value: crate::AlignItems) -> AlignItems {
    match value {
        crate::AlignItems::Center => AlignItems::CENTER,
        crate::AlignItems::FlexStart => AlignItems::FLEX_START,
        crate::AlignItems::FlexEnd => AlignItems::FLEX_END,
        crate::AlignItems::Stretch => AlignItems::STRETCH,
    }
}

fn typed_align_self(value: crate::AlignItems) -> taffy::style::AlignSelf {
    typed_align_items(value)
}

fn typed_overflow(value: Overflow) -> taffy::Overflow {
    match value {
        Overflow::Hidden => taffy::Overflow::Hidden,
        Overflow::Auto | Overflow::Scroll => taffy::Overflow::Scroll,
        Overflow::Visible => taffy::Overflow::Visible,
    }
}

fn typed_dimension(value: Length) -> Dimension {
    match value {
        Length::Auto => Dimension::auto(),
        Length::Px(value) => Dimension::length(value),
        Length::Percent(value) => Dimension::percent(value / 100.0),
    }
}

fn typed_dimension_auto(value: Length) -> taffy::LengthPercentageAuto {
    match value {
        Length::Auto => taffy::LengthPercentageAuto::auto(),
        Length::Px(value) => taffy::LengthPercentageAuto::length(value),
        Length::Percent(value) => taffy::LengthPercentageAuto::percent(value / 100.0),
    }
}

fn typed_edge_rect(values: [Option<Length>; 4]) -> taffy::Rect<LengthPercentage> {
    let values = values.map(|value| {
        value
            .and_then(typed_length)
            .unwrap_or_else(|| LengthPercentage::length(0.0))
    });
    taffy::Rect {
        top: values[0],
        right: values[1],
        bottom: values[2],
        left: values[3],
    }
}

fn typed_edge_rect_auto(values: [Option<Length>; 4]) -> taffy::Rect<taffy::LengthPercentageAuto> {
    let values = values.map(|value| match value {
        Some(Length::Auto) => taffy::LengthPercentageAuto::auto(),
        None => taffy::LengthPercentageAuto::length(0.0),
        Some(Length::Px(value)) => taffy::LengthPercentageAuto::length(value),
        Some(Length::Percent(value)) => taffy::LengthPercentageAuto::percent(value / 100.0),
    });
    taffy::Rect {
        top: values[0],
        right: values[1],
        bottom: values[2],
        left: values[3],
    }
}

fn typed_grid_tracks(values: &[GridTrack]) -> Vec<GridTemplateComponent<String>> {
    values
        .iter()
        .map(|track| {
            let sizing = match *track {
                GridTrack::Auto => taffy::style::TrackSizingFunction::AUTO,
                GridTrack::Length(Length::Px(value)) => {
                    taffy::style::TrackSizingFunction::from_length(value)
                }
                GridTrack::Length(Length::Percent(value)) => {
                    taffy::style::TrackSizingFunction::from_percent(value / 100.0)
                }
                GridTrack::Length(Length::Auto) => taffy::style::TrackSizingFunction::AUTO,
                GridTrack::Fraction(value) => fr(value),
                GridTrack::MinMax(min, max) => minmax(typed_min_track(min), typed_max_track(max)),
                GridTrack::MinMaxFraction(min, max) => minmax(
                    typed_min_track(min),
                    taffy::style::MaxTrackSizingFunction::from_fr(max),
                ),
                GridTrack::MinMaxAuto(min) => minmax(
                    typed_min_track(min),
                    taffy::style::MaxTrackSizingFunction::AUTO,
                ),
            };
            GridTemplateComponent::Single(sizing)
        })
        .collect()
}

fn typed_min_track(value: Length) -> taffy::style::MinTrackSizingFunction {
    match value {
        Length::Px(value) => taffy::style::MinTrackSizingFunction::from_length(value),
        Length::Percent(value) => taffy::style::MinTrackSizingFunction::percent(value / 100.0),
        Length::Auto => taffy::style::MinTrackSizingFunction::AUTO,
    }
}

fn typed_max_track(value: Length) -> taffy::style::MaxTrackSizingFunction {
    match value {
        Length::Px(value) => taffy::style::MaxTrackSizingFunction::from_length(value),
        Length::Percent(value) => taffy::style::MaxTrackSizingFunction::percent(value / 100.0),
        Length::Auto => taffy::style::MaxTrackSizingFunction::AUTO,
    }
}

/// Reports whether computed overflow enables scrolling.
pub fn is_scrollable(style: &ComputedStyle) -> bool {
    ["overflow", "overflow-x", "overflow-y"]
        .iter()
        .any(|property| {
            style.properties.get(*property).is_some_and(|value| {
                value.eq_ignore_ascii_case("auto") || value.eq_ignore_ascii_case("scroll")
            })
        })
}
