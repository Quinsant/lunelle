use lunelle_css::{
    Transform2D, TransformFunction, TransformLength, parse_transform, parse_transform_origin,
};

#[test]
fn parses_2d_transform_function_list() {
    let functions = parse_transform("translate(10px, 20px) rotate(90deg) scale(2)").unwrap();
    assert_eq!(functions.len(), 3);
    let matrix = Transform2D::from_functions(&functions, 0.0, 0.0);
    let (x, y) = matrix.apply(1.0, 0.0);
    assert!((x - 10.0).abs() < 0.0001);
    assert!((y - 22.0).abs() < 0.0001);
}

#[test]
fn supports_none_axis_functions_and_turn_angles() {
    assert_eq!(parse_transform("none"), Some(Vec::new()));
    let functions = parse_transform("translatex(8px) skewy(0.25turn)").unwrap();
    assert_eq!(
        functions[0],
        TransformFunction::Translate(TransformLength::Px(8.0), TransformLength::Px(0.0))
    );
    assert_eq!(
        functions[1],
        TransformFunction::Skew(0.0, std::f32::consts::FRAC_PI_2)
    );
}

#[test]
fn resolves_transform_origin_against_box_size() {
    assert_eq!(
        parse_transform_origin("right bottom", 100.0, 40.0),
        Some((100.0, 40.0))
    );
    assert_eq!(
        parse_transform_origin("25% 10px", 80.0, 40.0),
        Some((20.0, 10.0))
    );
}

#[test]
fn resolves_translate_percentages_against_box_dimensions() {
    let functions = parse_transform("translate(50%, 25%)").unwrap();
    let matrix = Transform2D::from_functions(&functions, 200.0, 80.0);
    assert_eq!(matrix.apply(0.0, 0.0), (100.0, 20.0));
}

#[test]
fn rejects_unsupported_and_invalid_transform_functions() {
    assert!(parse_transform("translate(2em)").is_none());
    assert!(parse_transform("perspective(20px)").is_none());
    assert!(parse_transform_origin("middle", 10.0, 10.0).is_none());
}
