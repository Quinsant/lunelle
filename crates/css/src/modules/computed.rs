use std::collections::BTreeMap;

use crate::utils::{
    align_items, css_number, edge_value, grid_auto_flow, grid_track, grid_tracks, integer,
    justify_content, length, length_number, number, placement, span,
};
use crate::{ComputedStyle, CssComputedState, Style, TextPresentation};

/// Assigns a parsed optional property directly to a typed style field.
macro_rules! set_property {
    ($values:expr, $properties:expr, $field:ident, $name:literal, $parser:expr) => {
        $values.$field = ($parser)($properties.get($name));
    };
}

/// Assigns a keyword property, using the value type's default on invalid input.
macro_rules! set_keyword {
    ($values:expr, $properties:expr, $field:ident, $name:literal, $parser:expr) => {
        $values.$field = $properties.get($name).and_then($parser).unwrap_or_default();
    };
}

/// Converts one grid track property without duplicating lookup boilerplate.
macro_rules! set_track {
    ($values:expr, $properties:expr, $field:ident, $name:literal) => {
        $values.$field = $properties.get($name).and_then(|value| grid_track(value));
    };
}

/// Expands a four-side CSS shorthand into top, right, bottom, and left fields.
macro_rules! set_edges {
    (
        $values:expr,
        $properties:expr,
        $shorthand:literal,
        $top:ident,
        $right:ident,
        $bottom:ident,
        $left:ident
    ) => {
        $values.$top = edge_value($properties, $shorthand, "top");
        $values.$right = edge_value($properties, $shorthand, "right");
        $values.$bottom = edge_value($properties, $shorthand, "bottom");
        $values.$left = edge_value($properties, $shorthand, "left");
    };
}

/// Reads a foreground property and falls back to its shorthand counterpart.
macro_rules! set_color {
    ($values:expr, $properties:expr, $field:ident, $name:literal, $fallback:literal) => {
        $values.$field = $properties
            .get($name)
            .or_else(|| $properties.get($fallback))
            .and_then(|value| crate::parse_color(value).map(crate::Color));
    };
}

/// Builds the typed state consumed by layout, text, and rendering backends.
pub(crate) fn from_computed(style: &ComputedStyle) -> CssComputedState {
    let properties = &style.properties;
    let mut values = Style::default();
    apply_layout(&mut values, properties);
    apply_box_model(&mut values, properties);
    let text = apply_text(&mut values, style, properties);

    CssComputedState {
        transform: properties
            .get("transform")
            .and_then(|value| crate::parse_transform(value)),
        transform_origin: properties
            .get("transform-origin")
            .and_then(|value| crate::parse_transform_origin(value, 100.0, 100.0)),
        text,
        values,
    }
}

/// Projects layout, flex, grid, positioning, and gap properties into `Style`.
fn apply_layout(values: &mut Style, properties: &BTreeMap<String, String>) {
    set_keyword!(values, properties, layout_mode, "display", |value| {
        crate::LayoutMode::from_keyword(value)
    });
    set_property!(values, properties, width, "width", length);
    set_property!(values, properties, height, "height", length);
    set_property!(values, properties, min_width, "min-width", length);
    set_property!(values, properties, max_width, "max-width", length);
    set_property!(values, properties, min_height, "min-height", length);
    set_property!(values, properties, max_height, "max-height", length);
    set_keyword!(
        values,
        properties,
        flex_direction,
        "flex-direction",
        |value| crate::FlexDirection::from_keyword(value)
    );
    set_keyword!(values, properties, visibility, "visibility", |value| {
        crate::Visibility::from_keyword(value)
    });
    let overflow = properties
        .get("overflow")
        .and_then(|value| crate::Overflow::from_keyword(value))
        .unwrap_or_default();
    values.overflow = overflow;
    values.overflow_x = properties
        .get("overflow-x")
        .and_then(|value| crate::Overflow::from_keyword(value))
        .unwrap_or(overflow);
    values.overflow_y = properties
        .get("overflow-y")
        .and_then(|value| crate::Overflow::from_keyword(value))
        .unwrap_or(overflow);
    set_keyword!(values, properties, flex_wrap, "flex-wrap", |value| {
        crate::FlexWrap::from_keyword(value)
    });
    set_property!(
        values,
        properties,
        justify_content,
        "justify-content",
        justify_content
    );
    set_property!(
        values,
        properties,
        align_content,
        "align-content",
        justify_content
    );
    set_property!(
        values,
        properties,
        justify_items,
        "justify-items",
        align_items
    );
    values.justify_self = properties
        .get("justify-self")
        .map(|value| align_items(Some(value)));
    set_property!(values, properties, align_items, "align-items", align_items);
    values.align_self = properties
        .get("align-self")
        .map(|value| align_items(Some(value)));
    set_property!(values, properties, flex_basis, "flex-basis", length);
    set_property!(values, properties, flex_grow, "flex-grow", |value| number(
        value
    )
    .unwrap_or(0.0));
    set_property!(values, properties, flex_shrink, "flex-shrink", |value| {
        number(value).unwrap_or(1.0)
    });
    if let Some(value) = properties.get("flex")
        && !properties.contains_key("flex-grow")
        && !properties.contains_key("flex-shrink")
        && !properties.contains_key("flex-basis")
    {
        let tokens = crate::utils::split_top_level_whitespace(value);
        match tokens.as_slice() {
            ["none"] => {
                values.flex_grow = 0.0;
                values.flex_shrink = 0.0;
                values.flex_basis = crate::Length::Auto;
            }
            ["auto"] => {
                values.flex_grow = 1.0;
                values.flex_shrink = 1.0;
                values.flex_basis = crate::Length::Auto;
            }
            ["initial"] => {
                values.flex_grow = 0.0;
                values.flex_shrink = 1.0;
                values.flex_basis = crate::Length::Auto;
            }
            [grow] => {
                if let Ok(grow) = grow.parse::<f32>() {
                    values.flex_grow = grow;
                    values.flex_shrink = 1.0;
                    values.flex_basis = crate::Length::Auto;
                } else if let Some(basis) = crate::utils::length_string(grow) {
                    values.flex_grow = 1.0;
                    values.flex_shrink = 1.0;
                    values.flex_basis = basis;
                }
            }
            [grow, shrink] => {
                if let (Ok(grow), Ok(shrink)) = (grow.parse::<f32>(), shrink.parse::<f32>()) {
                    values.flex_grow = grow;
                    values.flex_shrink = shrink;
                    values.flex_basis = crate::Length::Auto;
                }
            }
            [grow, shrink, basis] => {
                if let (Ok(grow), Ok(shrink), Some(basis)) = (
                    grow.parse::<f32>(),
                    shrink.parse::<f32>(),
                    crate::utils::length_string(basis),
                ) {
                    values.flex_grow = grow;
                    values.flex_shrink = shrink;
                    values.flex_basis = basis;
                }
            }
            _ => {}
        }
    }
    set_property!(
        values,
        properties,
        grid_template_columns,
        "grid-template-columns",
        grid_tracks
    );
    set_property!(
        values,
        properties,
        grid_template_rows,
        "grid-template-rows",
        grid_tracks
    );
    set_track!(values, properties, grid_auto_columns, "grid-auto-columns");
    set_track!(values, properties, grid_auto_rows, "grid-auto-rows");
    set_property!(
        values,
        properties,
        grid_auto_flow,
        "grid-auto-flow",
        grid_auto_flow
    );
    set_property!(
        values,
        properties,
        grid_column_start,
        "grid-column-start",
        integer
    );
    set_property!(
        values,
        properties,
        grid_row_start,
        "grid-row-start",
        integer
    );
    set_property!(
        values,
        properties,
        grid_column_span,
        "grid-column-span",
        span
    );
    set_property!(values, properties, grid_row_span, "grid-row-span", span);
    if let Some(value) = properties
        .get("grid-column")
        .and_then(|value| placement(value))
    {
        values.grid_column_start = value.0.or(values.grid_column_start);
        values.grid_column_end = value.1.or(values.grid_column_end);
        values.grid_column_span = value.2.or(values.grid_column_span);
    }
    if let Some(value) = properties
        .get("grid-row")
        .and_then(|value| placement(value))
    {
        values.grid_row_start = value.0.or(values.grid_row_start);
        values.grid_row_end = value.1.or(values.grid_row_end);
        values.grid_row_span = value.2.or(values.grid_row_span);
    }
    values.grid_area = properties.get("grid-area").cloned();
    set_keyword!(values, properties, position, "position", |value| {
        crate::Position::from_keyword(value)
    });
    set_property!(values, properties, left, "left", length);
    set_property!(values, properties, right, "right", length);
    set_property!(values, properties, top, "top", length);
    set_property!(values, properties, bottom, "bottom", length);
    values.row_gap = properties
        .get("row-gap")
        .or_else(|| properties.get("gap"))
        .and_then(|value| css_number(value));
    values.column_gap = properties
        .get("column-gap")
        .or_else(|| properties.get("gap"))
        .and_then(|value| css_number(value));
    values.gap = values.row_gap.or(values.column_gap).unwrap_or(0.0);
}

/// Projects margin, padding, border, background, and overflow properties.
fn apply_box_model(values: &mut Style, properties: &BTreeMap<String, String>) {
    set_edges!(
        values,
        properties,
        "margin",
        margin_top,
        margin_right,
        margin_bottom,
        margin_left
    );
    values.margin = values.margin_top.and_then(length_number).unwrap_or(0.0);
    set_edges!(
        values,
        properties,
        "padding",
        padding_top,
        padding_right,
        padding_bottom,
        padding_left
    );
    values.padding = values.padding_top.and_then(length_number).unwrap_or(0.0);
    values.border_width = edge_value(properties, "border-width", "top")
        .or_else(|| edge_value(properties, "border", "top"))
        .and_then(length_number)
        .unwrap_or(0.0);
    set_color!(values, properties, border_color, "border-color", "border");
    set_color!(
        values,
        properties,
        background,
        "background-color",
        "background"
    );
}

/// Projects text properties and returns the renderer-facing text presentation.
fn apply_text(
    values: &mut Style,
    style: &ComputedStyle,
    properties: &BTreeMap<String, String>,
) -> TextPresentation {
    set_keyword!(values, properties, text_align, "text-align", |value| {
        crate::TextAlign::from_keyword(value)
    });
    let text = crate::text_presentation(style);
    values.font_size = Some(text.font_size);
    values.line_height = Some(crate::LineHeight::Length(text.line_height));
    values.font_weight = text.weight;
    values.font_style = properties
        .get("font-style")
        .and_then(|value| crate::FontStyle::from_keyword(value));
    values.text_wrap = properties
        .get("white-space")
        .map(|value| !matches!(value.as_str(), "nowrap" | "pre"));
    text
}
