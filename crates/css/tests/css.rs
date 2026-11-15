use lunelle_core::NodeId;
use lunelle_css::{
    AnimationDirection, AnimationFillMode, AnimationIterationCount, CssRender, Length, StyleNode,
    Transform2D, TransformFunction, TransformLength, parse_animation, parse_transform,
    resolve_vars,
};

#[test]
fn css_render_computes_styles_from_a_stylesheet() {
    let css = CssRender::parse("button { color: red; }");
    let button = StyleNode {
        id: NodeId::from_raw(101),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };

    assert_eq!(css.compute(&[button])[0].properties["color"], "red");
}

#[test]
fn animation_parser_preserves_shorthand_values() {
    let animations = parse_animation("pulse 1.5s ease-in-out 200ms 3 alternate both paused")
        .expect("valid animation shorthand");

    assert_eq!(animations[0].name, "pulse");
    assert_eq!(animations[0].duration_ms, 1500.0);
    assert_eq!(animations[0].delay_ms, 200.0);
    assert_eq!(
        animations[0].iteration_count,
        AnimationIterationCount::Count(3.0)
    );
    assert_eq!(animations[0].direction, AnimationDirection::Alternate);
    assert_eq!(animations[0].fill_mode, AnimationFillMode::Both);
}

#[test]
fn transform_parser_and_matrix_use_box_dimensions() {
    let functions = parse_transform("translate(50%, 25%)").expect("valid transform");
    assert_eq!(
        functions,
        vec![TransformFunction::Translate(
            TransformLength::Percent(50.0),
            TransformLength::Percent(25.0),
        )]
    );

    let matrix = Transform2D::from_functions(&functions, 200.0, 80.0);
    assert_eq!(matrix.apply(0.0, 0.0), (100.0, 20.0));
}

#[test]
fn custom_properties_resolve_nested_fallbacks() {
    let properties = std::collections::HashMap::from([("--accent".to_owned(), "teal".to_owned())]);

    assert_eq!(
        resolve_vars("var(--missing, var(--accent, red))", &properties, 0),
        Some("teal".to_owned())
    );
}

#[test]
fn invalid_transform_and_custom_property_values_are_rejected() {
    assert!(parse_transform("translate(2em)").is_none());
    assert_eq!(
        resolve_vars("var(--missing)", &std::collections::HashMap::new(), 0),
        None
    );
}

#[test]
fn public_layout_values_are_constructible() {
    assert_eq!(Length::Px(12.0), Length::Px(12.0));
}
