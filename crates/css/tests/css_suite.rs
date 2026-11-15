use std::collections::BTreeMap;

use lunelle_core::NodeId;

use lunelle_css::*;

#[test]
fn css_render_is_the_stylesheet_and_computation_entrypoint() {
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
fn css_render_registers_standard_modules_and_property_owners() {
    let css = CssRender::default();
    let modules = css
        .modules()
        .modules()
        .map(CssModule::name)
        .collect::<Vec<_>>();

    assert_eq!(
        modules,
        [
            "base",
            "flexbox",
            "grid",
            "transform",
            "media",
            "property",
            "text"
        ]
    );
    assert_eq!(css.modules().module_for("width"), Some("base"));
    assert_eq!(css.modules().module_for("flex-grow"), Some("flexbox"));
    assert_eq!(css.modules().module_for("grid-area"), Some("grid"));
    assert_eq!(css.modules().module_for("transform"), Some("transform"));
    assert_eq!(css.modules().module_for("font-family"), Some("text"));
}

#[test]
fn css_render_accepts_a_custom_module_registry() {
    let mut modules = CssModuleRegistry::standard();
    modules.register(CssMedia::new());
    let css = CssRender::with_modules(StyleSheet::default(), modules);

    assert_eq!(css.modules().modules().count(), 8);
    assert_eq!(css.modules().module_for("width"), Some("base"));
}

#[test]
fn css_media_module_owns_condition_matching() {
    let conditions = vec![
        MediaCondition::MinWidth(600.0),
        MediaCondition::MaxHeight(800.0),
    ];
    let matching = CssMediaContext {
        viewport: lunelle_core::Size {
            width: 640.0,
            height: 720.0,
        },
        scale_factor: 1.0,
    };
    let non_matching = CssMediaContext {
        viewport: lunelle_core::Size {
            width: 480.0,
            height: 720.0,
        },
        scale_factor: 1.0,
    };

    assert!(CssMedia::matches(&conditions, Some(matching)));
    assert!(!CssMedia::matches(&conditions, Some(non_matching)));
    assert!(!CssMedia::matches(&conditions, None));
    assert!(CssMedia::matches(&[], None));
}

#[test]
fn css_module_state_round_trips_compute_context() {
    let node = NodeId::from_raw(108);
    let mut states = BTreeMap::new();
    states.insert(
        node,
        StyleState {
            hovered: true,
            ..StyleState::default()
        },
    );
    let context = StyleComputeContext {
        media: Some(CssMediaContext {
            viewport: lunelle_core::Size {
                width: 800.0,
                height: 600.0,
            },
            scale_factor: 2.0,
        }),
        snapshot_version: 3,
        stylesheet_version: 5,
        environment_version: 7,
        states,
    };

    let state = CssModuleState::from_context(&context);
    assert_eq!(state.context(), context);
}

#[test]
fn duplicate_module_ownership_is_reported_without_changing_dispatch() {
    let mut modules = CssModuleRegistry::standard();
    modules.register(CssBase::new());
    assert_eq!(modules.module_for("width"), Some("base"));
    assert!(
        modules
            .duplicate_owners()
            .iter()
            .any(|(property, first, duplicate)| *property == "width"
                && *first == "base"
                && *duplicate == "base")
    );

    let css = CssRender::with_modules(StyleSheet::default(), modules);
    let node = StyleNode {
        id: NodeId::from_raw(107),
        tag_name: "div".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let (_, diagnostics) = css.compute_with_diagnostics(&[node]);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("width"))
    );
}

#[test]
fn standard_declarations_validate_values_during_module_compute() {
    let css = CssRender::parse(
        "button { transform: translate(10px, 20px); opacity: 2; display: unsupported; }",
    );
    let button = StyleNode {
        id: NodeId::from_raw(102),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };

    let computed = css.compute(&[button]);
    assert_eq!(computed[0].properties["transform"], "translate(10px, 20px)");
    assert!(!computed[0].properties.contains_key("opacity"));
    assert!(!computed[0].properties.contains_key("display"));
}

#[test]
fn display_none_projects_to_taffy_none() {
    let css = CssRender::parse(".hidden { display: none; }");
    let hidden = StyleNode {
        id: NodeId::from_raw(110),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["hidden".into()],
        parent: None,
    };

    let computed = css.compute(&[hidden]);
    let state = css.modules().state(&computed[0]);
    let (style, _) = lunelle_css::taffy_style_state(&state);
    assert_eq!(style.display, taffy::Display::None);
}

#[test]
fn computed_state_projects_layout_transform_and_text_values() {
    let css = CssRender::parse(
        ".card { display: grid; width: 50%; min-height: 24px; flex-grow: 2; flex-shrink: 0; flex-basis: 12px; align-items: center; position: relative; left: 4px; transform: translate(4px, 8px); transform-origin: right bottom; font-size: 18px; line-height: 24px; font-weight: bold; font-style: italic; font-stretch: condensed; text-decoration: underline line-through; text-transform: uppercase; white-space: nowrap; }",
    );
    let card = StyleNode {
        id: NodeId::from_raw(103),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["card".into()],
        parent: None,
    };

    let computed = css.compute(&[card]);
    let state = css.modules().state(&computed[0]);

    assert_eq!(state.values.layout_mode, LayoutMode::Grid);
    assert_eq!(state.values.width, Length::Percent(50.0));
    assert_eq!(state.values.min_height, Length::Px(24.0));
    assert_eq!(state.values.flex_grow, 2.0);
    assert_eq!(state.values.flex_shrink, 0.0);
    assert_eq!(state.values.flex_basis, Length::Px(12.0));
    assert_eq!(state.values.align_items, AlignItems::Center);
    assert_eq!(state.values.position, Position::Relative);
    assert_eq!(state.values.left, Length::Px(4.0));
    assert_eq!(state.values.line_height, Some(LineHeight::Length(24.0)));
    assert_eq!(state.values.font_weight, Some(700));
    assert_eq!(state.values.font_style, Some(FontStyle::Italic));
    assert_eq!(state.text.stretch, FontStretch::Condensed);
    assert!(state.text.decoration.underline);
    assert!(state.text.decoration.line_through);
    assert!(!state.text.decoration.overline);
    assert_eq!(state.text.transform, TextTransform::Uppercase);
    assert_eq!(state.values.text_wrap, Some(false));
    assert!(state.transform.is_some());
    assert_eq!(state.transform_origin, Some((100.0, 100.0)));
    assert_eq!(state.text.font_size, 18.0);
}

#[test]
fn computed_state_projects_text_decoration_line_longhand() {
    let css = CssRender::parse(
        ".label { text-decoration-line: overline; text-decoration-color: #ff0000; text-decoration-style: double; text-decoration-thickness: 3px; }",
    );
    let label = StyleNode {
        id: NodeId::from_raw(109),
        tag_name: "span".into(),
        element_id: None,
        classes: vec!["label".into()],
        parent: None,
    };

    let computed = css.compute(&[label]);
    let state = css.modules().state(&computed[0]);
    assert!(state.text.decoration.overline);
    assert!(!state.text.decoration.underline);
    assert!(!state.text.decoration.line_through);
    assert_eq!(state.text.decoration.color, [255, 0, 0, 255]);
    assert_eq!(state.text.decoration.style, TextDecorationStyle::Double);
    assert_eq!(
        state.text.decoration.thickness,
        TextDecorationThickness::Px(3.0)
    );
}

#[test]
fn computed_state_projects_flex_wrap_and_shorthand() {
    let css = CssRender::parse(".row { display: flex; flex-wrap: wrap-reverse; flex: 2 0 12px; }");
    let row = StyleNode {
        id: NodeId::from_raw(106),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["row".into()],
        parent: None,
    };

    let computed = css.compute(&[row]);
    let state = css.modules().state(&computed[0]);

    assert_eq!(state.values.flex_wrap, FlexWrap::WrapReverse);
    assert_eq!(state.values.flex_grow, 2.0);
    assert_eq!(state.values.flex_shrink, 0.0);
    assert_eq!(state.values.flex_basis, Length::Px(12.0));
}

#[test]
fn flex_longhands_follow_declaration_order() {
    let first = CssRender::parse(".item { flex-grow: 2; flex: 1; }");
    let second = CssRender::parse(".item { flex: 1; flex-grow: 2; }");
    let item = StyleNode {
        id: NodeId::from_raw(111),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["item".into()],
        parent: None,
    };

    assert_eq!(
        first
            .modules()
            .state(&first.compute(std::slice::from_ref(&item))[0])
            .values
            .flex_grow,
        1.0
    );
    assert_eq!(
        second
            .modules()
            .state(&second.compute(std::slice::from_ref(&item))[0])
            .values
            .flex_grow,
        2.0
    );
}

#[test]
fn computed_state_projects_box_alignment_values() {
    let css = CssRender::parse(
        ".grid { display: grid; align-content: space-between; justify-items: center; justify-self: end; }",
    );
    let grid = StyleNode {
        id: NodeId::from_raw(107),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["grid".into()],
        parent: None,
    };

    let computed = css.compute(&[grid]);
    let state = css.modules().state(&computed[0]);

    assert_eq!(state.values.align_content, JustifyContent::SpaceBetween);
    assert_eq!(state.values.justify_items, AlignItems::Center);
    assert_eq!(state.values.justify_self, Some(AlignItems::FlexEnd));
}

#[test]
fn taffy_style_projects_position_inset_align_self_and_grid_placement() {
    let css = CssRender::parse(
        ".item { position: absolute; top: 8px; right: 12px; align-self: end; grid-column: 2 / span 3; grid-row: 1 / 4; }",
    );
    let item = StyleNode {
        id: NodeId::from_raw(112),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["item".into()],
        parent: None,
    };

    let state = css.modules().state(&css.compute(&[item])[0]);
    assert_eq!(state.values.grid_row_end, Some(4));
    let (style, _) = taffy_style_state(&state);

    assert_eq!(style.position, taffy::Position::Absolute);
    assert_eq!(style.inset.top, taffy::LengthPercentageAuto::length(8.0));
    assert_eq!(style.inset.right, taffy::LengthPercentageAuto::length(12.0));
    assert_eq!(style.inset.bottom, taffy::LengthPercentageAuto::auto());
    assert_eq!(style.align_self, Some(taffy::style::AlignSelf::FLEX_END));
    assert!(
        matches!(style.grid_column.start, taffy::style::GridPlacement::Line(line) if line == 2i16.into())
    );
    assert!(matches!(
        style.grid_column.end,
        taffy::style::GridPlacement::Span(3)
    ));
    assert!(
        matches!(style.grid_row.start, taffy::style::GridPlacement::Line(line) if line == 1i16.into())
    );
    assert!(
        matches!(style.grid_row.end, taffy::style::GridPlacement::Line(line) if line == 4i16.into())
    );
}

#[test]
fn taffy_style_normalizes_css_percentages() {
    let css = CssRender::parse(".item { width: 50%; left: 25%; }");
    let item = StyleNode {
        id: NodeId::from_raw(115),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["item".into()],
        parent: None,
    };

    let state = css.modules().state(&css.compute(&[item])[0]);
    let (style, _) = taffy_style_state(&state);

    assert_eq!(style.size.width, taffy::Dimension::percent(0.5));
    assert_eq!(style.inset.left, taffy::LengthPercentageAuto::percent(0.25));
}

#[test]
fn computed_state_projects_inherited_visibility() {
    let css = CssRender::parse(".parent { visibility: hidden; } .child { display: block; }");
    let nodes = [
        StyleNode {
            id: NodeId::from_raw(108),
            tag_name: "div".into(),
            element_id: None,
            classes: vec!["parent".into()],
            parent: None,
        },
        StyleNode {
            id: NodeId::from_raw(109),
            tag_name: "span".into(),
            element_id: None,
            classes: vec!["child".into()],
            parent: Some(NodeId::from_raw(108)),
        },
    ];

    let states = css.states(&css.compute(&nodes));
    assert_eq!(states[0].values.visibility, Visibility::Hidden);
    assert_eq!(states[1].values.visibility, Visibility::Hidden);
}

#[test]
fn computed_state_projects_inherited_text_transform() {
    let css = CssRender::parse(".parent { text-transform: uppercase; }");
    let nodes = [
        StyleNode {
            id: NodeId::from_raw(113),
            tag_name: "div".into(),
            element_id: None,
            classes: vec!["parent".into()],
            parent: None,
        },
        StyleNode {
            id: NodeId::from_raw(114),
            tag_name: "span".into(),
            element_id: None,
            classes: Vec::new(),
            parent: Some(NodeId::from_raw(113)),
        },
    ];

    let states = css.states(&css.compute(&nodes));
    assert_eq!(states[0].text.transform, TextTransform::Uppercase);
    assert_eq!(states[1].text.transform, TextTransform::Uppercase);
}

#[test]
fn computed_state_projects_axis_overflow_with_shorthand_fallback() {
    let css = CssRender::parse(".panel { overflow: hidden; overflow-y: auto; }");
    let panel = StyleNode {
        id: NodeId::from_raw(110),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["panel".into()],
        parent: None,
    };

    let state = css.modules().state(&css.compute(&[panel])[0]);
    assert_eq!(state.values.overflow_x, Overflow::Hidden);
    assert_eq!(state.values.overflow_y, Overflow::Auto);

    #[cfg(feature = "layout-taffy")]
    {
        let (_, flags) = taffy_style_state(&state);
        assert!(flags.clips_children);
        assert!(flags.scrollable);
    }
}

#[test]
fn css_render_projects_states_in_computed_style_order() {
    let css = CssRender::parse(".grid { display: grid; } .small { font-size: 12px; }");
    let nodes = [
        StyleNode {
            id: NodeId::from_raw(104),
            tag_name: "div".into(),
            element_id: None,
            classes: vec!["grid".into()],
            parent: None,
        },
        StyleNode {
            id: NodeId::from_raw(105),
            tag_name: "span".into(),
            element_id: None,
            classes: vec!["small".into()],
            parent: Some(NodeId::from_raw(104)),
        },
    ];

    let computed = css.compute(&nodes);
    let states = css.states(&computed);

    assert_eq!(states.len(), 2);
    assert_eq!(states[0].values.layout_mode, LayoutMode::Grid);
    assert_eq!(states[1].text.font_size, 12.0);
}

#[test]
fn computed_state_projects_grid_tracks_and_placement() {
    let css = CssRender::parse(
        ".grid { display: grid; grid-template-columns: repeat(2, 1fr); grid-template-rows: minmax(24px, 2fr); grid-auto-flow: column dense; grid-column: 2 / span 3; grid-row: 1 / 4; grid-area: content; }",
    );
    let grid = StyleNode {
        id: NodeId::from_raw(106),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["grid".into()],
        parent: None,
    };

    let computed = css.compute(&[grid]);
    let state = css.modules().state(&computed[0]);

    assert_eq!(
        state.values.grid_template_columns,
        vec![GridTrack::Fraction(1.0), GridTrack::Fraction(1.0)]
    );
    assert_eq!(
        state.values.grid_template_rows,
        vec![GridTrack::MinMaxFraction(Length::Px(24.0), 2.0)]
    );
    assert_eq!(state.values.grid_auto_flow, GridAutoFlow::ColumnDense);
    assert_eq!(state.values.grid_column_start, Some(2));
    assert_eq!(state.values.grid_column_span, Some(3));
    assert_eq!(state.values.grid_row_start, Some(1));
    assert_eq!(state.values.grid_row_span, Some(3));
    assert_eq!(state.values.grid_area.as_deref(), Some("content"));
}

#[test]
fn computed_state_projects_base_spacing_and_colors() {
    let css = CssRender::parse(
        ".box { gap: 8px; margin: 1px 2px 3px 4px; padding: 5px 6px; border: 2px solid red; background: #102030; }",
    );
    let node = StyleNode {
        id: NodeId::from_raw(109),
        tag_name: "div".into(),
        element_id: None,
        classes: vec!["box".into()],
        parent: None,
    };

    let computed = css.compute(&[node]);
    let state = css.modules().state(&computed[0]);

    assert_eq!(state.values.gap, 8.0);
    assert_eq!(state.values.margin_top, Some(Length::Px(1.0)));
    assert_eq!(state.values.margin_right, Some(Length::Px(2.0)));
    assert_eq!(state.values.margin_bottom, Some(Length::Px(3.0)));
    assert_eq!(state.values.margin_left, Some(Length::Px(4.0)));
    assert_eq!(state.values.padding_top, Some(Length::Px(5.0)));
    assert_eq!(state.values.padding_right, Some(Length::Px(6.0)));
    assert_eq!(state.values.padding_bottom, Some(Length::Px(5.0)));
    assert_eq!(state.values.padding_left, Some(Length::Px(6.0)));
    assert_eq!(state.values.border_width, 2.0);
    assert_eq!(
        state.values.background,
        Some(Color([16.0 / 255.0, 32.0 / 255.0, 48.0 / 255.0, 1.0]))
    );
}

#[test]
fn converts_rem_lengths_without_changing_other_values() {
    assert_eq!(
        rem_to_px("padding: 1rem 0.5rem; margin-left: -2rem; opacity: 0.8; width: 12px;"),
        "padding: 16px 8px; margin-left: -32px; opacity: 0.8; width: 12px;",
    );
}

#[test]
fn recognizes_profile_and_custom_properties() {
    assert!(is_known_property("display"));
    assert!(is_known_property("--accent"));
    assert!(is_known_property("transition"));
    assert!(is_known_property("transition-duration"));
    assert!(is_known_property("animation-name"));
    assert!(is_known_property("animation-iteration-count"));
    assert!(!is_known_property("unknown-property"));
}

#[test]
fn parses_keyframes_with_from_to_and_percentage_offsets() {
    let sheet = StyleSheet::parse(
        "@keyframes pulse { from { opacity: 0; } 25%, 50% { opacity: .5; } to { opacity: 1; } }",
    );

    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "pulse");
    let frames = &sheet.keyframes[0].frames;
    assert_eq!(frames.len(), 4);
    assert_eq!(frames[0].offset, 0.0);
    assert_eq!(frames[1].offset, 0.25);
    assert_eq!(frames[2].offset, 0.5);
    assert_eq!(frames[3].offset, 1.0);
    assert_eq!(frames[2].declarations[0].name, "opacity");
    assert!(sheet.diagnostics.is_empty());
}

#[test]
fn malformed_keyframe_selector_is_diagnosed_without_losing_other_frames() {
    let sheet = StyleSheet::parse(
        "@keyframes pulse { 120% { opacity: 0; } 0% { opacity: 1; } } button { animation: pulse 1s; }",
    );

    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].frames.len(), 1);
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].declarations[0].name, "animation");
    assert_eq!(sheet.diagnostics.len(), 1);
    assert!(sheet.diagnostics[0].message.contains("keyframe selector"));
}

#[test]
fn loads_stylesheet_within_resource_root_and_rejects_traversal() {
    let root = std::env::temp_dir().join(format!(
        "lunelle-css-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("app.css"), "button { color: red; }").unwrap();

    let sheet = StyleSheet::load(&root, "app.css").unwrap();
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].declarations[0].value, "red");
    assert!(matches!(
        StyleSheet::load(&root, "../outside.css"),
        Err(CssLoadError::OutsideResourceRoot)
    ));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn parses_declarations_and_important() {
    let sheet = StyleSheet::parse("button { color: red; display: flex !important; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].declarations.len(), 2);
    assert!(sheet.rules[0].declarations[1].important);
}

#[test]
fn malformed_declarations_are_reported_and_valid_declarations_recover() {
    let sheet = StyleSheet::parse("button { color red; : blue; display: flex; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].declarations.len(), 1);
    assert_eq!(sheet.rules[0].declarations[0].name, "display");
    assert_eq!(sheet.diagnostics.len(), 2);
    assert!(sheet.diagnostics[0].message.contains("expected `:`"));
    assert!(
        sheet.diagnostics[1]
            .message
            .contains("expected a property name")
    );
}

#[test]
fn parses_nested_media_rules_and_preserves_source_order() {
    let sheet = StyleSheet::parse(
        "button { width: 10px; } @media (min-width: 600px) { button { width: 30px !important; } }",
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(sheet.rules[0].source_order, 0);
    assert!(sheet.rules[0].media_conditions.is_empty());
    assert_eq!(sheet.rules[1].source_order, 1);
    assert_eq!(sheet.rules[1].media_conditions.len(), 1);
    assert!(sheet.rules[1].declarations[0].important);
    assert!(sheet.diagnostics.is_empty());
}

#[test]
fn computes_child_selector_cascade_and_inherited_properties() {
    let sheet = StyleSheet::parse(
        "main { color: green; } button { color: black; } .primary { color: blue; } main > button.primary { color: red; font-size: 14px; }",
    );
    let root = StyleNode {
        id: NodeId::from_raw(1),
        tag_name: "main".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let child = StyleNode {
        id: NodeId::from_raw(2),
        tag_name: "button".into(),
        element_id: None,
        classes: vec!["primary".into()],
        parent: Some(root.id),
    };
    let computed = CssRender::new(sheet.clone()).compute(&[root, child]);
    assert_eq!(computed[1].properties["color"], "red");
    assert_eq!(computed[1].properties["font-size"], "14px");
}

#[test]
fn computes_opacity_property() {
    let sheet = StyleSheet::parse("button { opacity: 0.4; }");
    let button = StyleNode {
        id: NodeId::from_raw(31),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let computed = CssRender::new(sheet.clone()).compute(&[button]);
    assert_eq!(computed[0].properties["opacity"], "0.4");
}

#[test]
fn resolves_inherited_custom_properties_and_discards_invalid_var_values() {
    let sheet = StyleSheet::parse(
        "main { --accent: teal; --shade: var(--accent); color: var(--accent); } button { --accent: blue; color: var(--shade); background-color: var(--missing); border-color: VAR(--missing, orange); }",
    );
    let root = StyleNode {
        id: NodeId::from_raw(51),
        tag_name: "main".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let child = StyleNode {
        id: NodeId::from_raw(52),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: Some(root.id),
    };

    let (computed, diagnostics) =
        CssRender::new(sheet.clone()).compute_with_diagnostics(&[root, child]);
    assert_eq!(computed[0].properties["color"], "teal");
    assert_eq!(computed[1].properties["--shade"], "teal");
    assert_eq!(computed[1].properties["color"], "teal");
    assert!(!computed[1].properties.contains_key("background-color"));
    assert_eq!(computed[1].properties["border-color"], "orange");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == 2
                && diagnostic.message.contains("background-color")
                && diagnostic.message.contains("declaration discarded")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn resolves_css_wide_keywords_for_inherited_and_initial_values() {
    let sheet = StyleSheet::parse(
        "main { color: green; opacity: 0.4; } button { color: unset; opacity: unset; } .reset { color: initial; opacity: initial; }",
    );
    let root = StyleNode {
        id: NodeId::from_raw(41),
        tag_name: "main".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let child = StyleNode {
        id: NodeId::from_raw(42),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: Some(root.id),
    };
    let reset = StyleNode {
        id: NodeId::from_raw(43),
        tag_name: "button".into(),
        element_id: None,
        classes: vec!["reset".into()],
        parent: Some(root.id),
    };

    let computed = CssRender::new(sheet.clone()).compute(&[root, child, reset]);
    assert_eq!(computed[1].properties["color"], "green");
    assert_eq!(computed[1].properties["opacity"], "1");
    assert!(!computed[2].properties.contains_key("color"));
    assert_eq!(computed[2].properties["opacity"], "1");
}

#[test]
fn revert_discards_author_value_and_uses_inherited_or_initial_value() {
    let sheet = StyleSheet::parse(
        "main { color: green; opacity: 0.4; } button { color: red; color: revert; opacity: 0.2; opacity: revert; }",
    );
    let root = StyleNode {
        id: NodeId::from_raw(43),
        tag_name: "main".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let button = StyleNode {
        id: NodeId::from_raw(44),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: Some(root.id),
    };

    let result = CssRender::new(sheet.clone()).compute_with_diagnostics(&[root, button]);
    assert_eq!(result.0[1].properties["color"], "green");
    assert_eq!(result.0[1].properties["opacity"], "1");
    assert!(result.1.is_empty());
}

#[test]
fn inheritance_is_independent_of_snapshot_order_and_hover_uses_snapshot_state() {
    let sheet = StyleSheet::parse("main { color: green; } button:hover { color: red; }");
    let parent = StyleNode {
        id: NodeId::from_raw(21),
        tag_name: "main".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let child = StyleNode {
        id: NodeId::from_raw(22),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: Some(parent.id),
    };
    let context = StyleComputeContext {
        snapshot_version: 7,
        stylesheet_version: 11,
        environment_version: 13,
        states: BTreeMap::from([(
            child.id,
            StyleState {
                hovered: true,
                ..StyleState::default()
            },
        )]),
        ..StyleComputeContext::default()
    };
    let result = CssRender::new(sheet.clone()).compute_with_context(&[child, parent], &context);
    assert_eq!(result.styles[0].properties["color"], "red");
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.snapshot_version, 7);
    assert_eq!(result.stylesheet_version, 11);
    assert_eq!(result.environment_version, 13);
}

#[test]
fn media_rules_only_apply_when_the_context_matches() {
    let sheet = StyleSheet::parse(
        "button { width: 10px; } @media (min-width: 600px) { button { width: 30px; } }",
    );
    let button = StyleNode {
        id: NodeId::from_raw(31),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let small = StyleComputeContext {
        media: Some(CssMediaContext {
            viewport: lunelle_core::Size {
                width: 500.0,
                height: 400.0,
            },
            scale_factor: 1.0,
        }),
        ..StyleComputeContext::default()
    };
    let large = StyleComputeContext {
        media: Some(CssMediaContext {
            viewport: lunelle_core::Size {
                width: 800.0,
                height: 600.0,
            },
            scale_factor: 1.0,
        }),
        ..small.clone()
    };

    assert_eq!(
        CssRender::new(sheet.clone())
            .compute_with_context(&[button.clone()], &small)
            .styles[0]
            .properties["width"],
        "10px"
    );
    assert_eq!(
        CssRender::new(sheet.clone())
            .compute_with_context(&[button], &large)
            .styles[0]
            .properties["width"],
        "30px"
    );
}

#[test]
fn media_conditions_accept_css_whitespace_and_case_variations() {
    let sheet = StyleSheet::parse(
        "@media (min-width: 600px)  AND\n (max-height: 700px) { button { width: 30px; } }",
    );
    let button = StyleNode {
        id: NodeId::from_raw(32),
        tag_name: "button".into(),
        element_id: None,
        classes: Vec::new(),
        parent: None,
    };
    let context = StyleComputeContext {
        media: Some(CssMediaContext {
            viewport: lunelle_core::Size {
                width: 800.0,
                height: 600.0,
            },
            scale_factor: 1.0,
        }),
        ..StyleComputeContext::default()
    };

    let result = CssRender::new(sheet.clone()).compute_with_context(&[button], &context);
    assert_eq!(result.styles[0].properties["width"], "30px");
    assert!(result.diagnostics.is_empty());
}
