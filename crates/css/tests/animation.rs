use std::collections::BTreeMap;

use lunelle_css::{
    Animation, AnimationController, AnimationDirection, AnimationFillMode, AnimationIterationCount,
    AnimationPlayState, AnimationRun, AnimationSample, ComputedStyle, ComputedStyleTree,
    StyleSheet, TimingFunction, parse_animation, resolve_animations, sample_animation_runs,
    sample_keyframes,
};

#[test]
fn parses_animation_shorthand_and_comma_separated_lists() {
    let animations = parse_animation(
        "pulse 1.5s ease-in-out 200ms 3 alternate both paused, \"fade-in\" 250ms linear infinite reverse forwards",
    )
    .unwrap();

    assert_eq!(animations.len(), 2);
    assert_eq!(animations[0].name, "pulse");
    assert_eq!(animations[0].duration_ms, 1500.0);
    assert_eq!(animations[0].delay_ms, 200.0);
    assert_eq!(
        animations[0].iteration_count,
        AnimationIterationCount::Count(3.0)
    );
    assert_eq!(animations[0].direction, AnimationDirection::Alternate);
    assert_eq!(animations[0].fill_mode, AnimationFillMode::Both);
    assert_eq!(animations[0].play_state, AnimationPlayState::Paused);
    assert_eq!(animations[1].name, "fade-in");
    assert_eq!(
        animations[1].iteration_count,
        AnimationIterationCount::Infinite
    );
    assert_eq!(animations[1].direction, AnimationDirection::Reverse);
}

#[test]
fn applies_animation_defaults_and_rejects_invalid_or_trailing_items() {
    assert_eq!(
        parse_animation("pulse").unwrap()[0],
        Animation {
            name: "pulse".into(),
            ..Animation::default()
        }
    );
    assert_eq!(parse_animation("none").unwrap()[0].name, "none");
    for invalid in [
        "pulse -1s",
        "pulse 1s 2s 3s",
        "pulse -2",
        "pulse 1s,",
        "pulse 1s pulse",
    ] {
        assert!(parse_animation(invalid).is_none(), "accepted {invalid}");
    }
}

#[test]
fn resolves_computed_animation_longhands_and_repeats_short_lists() {
    let properties = BTreeMap::from([
        ("animation-name".into(), "pulse, fade".into()),
        ("animation-duration".into(), "1s".into()),
        ("animation-delay".into(), "-100ms, 200ms".into()),
        ("animation-iteration-count".into(), "2, infinite".into()),
        ("animation-direction".into(), "alternate".into()),
        ("animation-fill-mode".into(), "both".into()),
        ("animation-play-state".into(), "paused, running".into()),
    ]);

    let animations = resolve_animations(&properties).unwrap();
    assert_eq!(animations.len(), 2);
    assert_eq!(animations[0].name, "pulse");
    assert_eq!(animations[0].duration_ms, 1000.0);
    assert_eq!(animations[0].delay_ms, -100.0);
    assert_eq!(
        animations[0].iteration_count,
        AnimationIterationCount::Count(2.0)
    );
    assert_eq!(animations[0].direction, AnimationDirection::Alternate);
    assert_eq!(animations[0].fill_mode, AnimationFillMode::Both);
    assert_eq!(animations[0].play_state, AnimationPlayState::Paused);
    assert_eq!(animations[1].name, "fade");
    assert_eq!(animations[1].duration_ms, 1000.0);
    assert_eq!(animations[1].delay_ms, 200.0);
    assert_eq!(
        animations[1].iteration_count,
        AnimationIterationCount::Infinite
    );
    assert_eq!(animations[1].play_state, AnimationPlayState::Running);
}

#[test]
fn longhand_animation_name_overrides_shorthand_and_invalid_lists_fail() {
    let properties = BTreeMap::from([
        ("animation".into(), "old 1s, second 2s".into()),
        ("animation-name".into(), "new, final, extra".into()),
        ("animation-duration".into(), "250ms, 500ms".into()),
    ]);

    let animations = resolve_animations(&properties).unwrap();
    assert_eq!(animations.len(), 3);
    assert_eq!(animations[0].name, "new");
    assert_eq!(animations[0].duration_ms, 250.0);
    assert_eq!(animations[1].name, "final");
    assert_eq!(animations[1].duration_ms, 500.0);
    assert_eq!(animations[2].name, "extra");
    assert_eq!(animations[2].duration_ms, 250.0);

    let invalid = BTreeMap::from([
        ("animation-name".into(), "pulse".into()),
        ("animation-duration".into(), "-1s".into()),
    ]);
    assert!(resolve_animations(&invalid).is_none());
    assert!(resolve_animations(&BTreeMap::new()).unwrap().is_empty());
}

#[test]
fn timeline_samples_delay_iterations_direction_and_fill() {
    let mut animation = Animation {
        duration_ms: 100.0,
        delay_ms: 50.0,
        iteration_count: AnimationIterationCount::Count(2.5),
        direction: AnimationDirection::Alternate,
        fill_mode: AnimationFillMode::Both,
        timing_function: TimingFunction::Linear,
        ..Animation::default()
    };
    animation.name = "pulse".into();
    let run = AnimationRun::new(animation, 10.0);

    assert_eq!(
        run.sample(20.0).unwrap(),
        AnimationSample {
            progress: 0.0,
            iteration: 0,
            finished: false,
        }
    );
    assert_eq!(run.sample(110.0).unwrap().progress, 0.5);
    assert_eq!(run.sample(160.0).unwrap().progress, 1.0);
    assert_eq!(
        run.sample(360.0).unwrap(),
        AnimationSample {
            progress: 0.5,
            iteration: 2,
            finished: true,
        }
    );
}

#[test]
fn timeline_freezes_when_paused_and_accounts_for_resume() {
    let animation = Animation {
        name: "pulse".into(),
        duration_ms: 100.0,
        timing_function: TimingFunction::Linear,
        ..Animation::default()
    };
    let mut run = AnimationRun::new(animation, 0.0);
    run.pause(25.0);
    assert_eq!(run.sample(80.0).unwrap().progress, 0.25);
    run.resume(75.0);
    assert_eq!(run.sample(100.0).unwrap().progress, 0.5);
}

#[test]
fn zero_iterations_respect_both_fill_during_delay_and_afterwards() {
    let animation = Animation {
        name: "pulse".into(),
        duration_ms: 100.0,
        delay_ms: 50.0,
        iteration_count: AnimationIterationCount::Count(0.0),
        fill_mode: AnimationFillMode::Both,
        timing_function: TimingFunction::Linear,
        ..Animation::default()
    };
    let run = AnimationRun::new(animation, 0.0);

    assert_eq!(run.sample(25.0).unwrap().progress, 0.0);
    assert_eq!(run.sample(75.0).unwrap().progress, 1.0);
    assert!(run.sample(75.0).unwrap().finished);
}

#[test]
fn samples_numeric_keyframe_values_and_implicit_underlying_endpoints() {
    let sheet = StyleSheet::parse(
        "@keyframes move { 25% { left: 20px; opacity: .2; } 75% { left: 60px; opacity: .8; } }",
    );
    let keyframes = &sheet.keyframes[0];
    let underlying = BTreeMap::from([
        ("left".to_owned(), "0px".to_owned()),
        ("opacity".to_owned(), "1".to_owned()),
    ]);

    assert_eq!(sample_keyframes(keyframes, 0.0, &underlying)["left"], "0px");
    assert_eq!(
        sample_keyframes(keyframes, 0.5, &underlying)["left"],
        "40px"
    );
    assert_eq!(
        sample_keyframes(keyframes, 0.5, &underlying)["opacity"],
        "0.5"
    );
    assert_eq!(sample_keyframes(keyframes, 1.0, &underlying)["left"], "0px");
}

#[test]
fn keyframe_sampler_uses_discrete_fallback_and_ignores_non_finite_progress() {
    let sheet = StyleSheet::parse(
        "@keyframes visibility { from { display: block; } to { display: none; } }",
    );
    let keyframes = &sheet.keyframes[0];
    let underlying = BTreeMap::new();

    assert_eq!(
        sample_keyframes(keyframes, 0.49, &underlying)["display"],
        "block"
    );
    assert_eq!(
        sample_keyframes(keyframes, 0.5, &underlying)["display"],
        "none"
    );
    assert!(sample_keyframes(keyframes, f32::NAN, &underlying).is_empty());
}

#[test]
fn samples_named_animation_runs_and_later_runs_override_same_property() {
    let sheet = StyleSheet::parse(
        "@keyframes move { from { left: 0px; opacity: 0; } to { left: 100px; opacity: 1; } } \
         @keyframes fade { from { opacity: .2; } to { opacity: .8; } }",
    );
    let runs = [
        AnimationRun::new(
            Animation {
                name: "move".into(),
                duration_ms: 100.0,
                timing_function: TimingFunction::Linear,
                ..Animation::default()
            },
            0.0,
        ),
        AnimationRun::new(
            Animation {
                name: "fade".into(),
                duration_ms: 100.0,
                timing_function: TimingFunction::Linear,
                ..Animation::default()
            },
            0.0,
        ),
    ];
    let underlying = BTreeMap::from([
        ("left".into(), "10px".into()),
        ("opacity".into(), "1".into()),
    ]);

    let sampled = sample_animation_runs(&runs, &sheet.keyframes, 50.0, &underlying);
    assert_eq!(sampled["left"], "50px");
    assert_eq!(sampled["opacity"], "0.5");
}

#[test]
fn missing_keyframes_and_inactive_runs_make_no_contribution() {
    let sheet = StyleSheet::parse("@keyframes pulse { from { opacity: 0; } to { opacity: 1; } }");
    let runs = [
        AnimationRun::new(
            Animation {
                name: "missing".into(),
                duration_ms: 100.0,
                ..Animation::default()
            },
            0.0,
        ),
        AnimationRun::new(
            Animation {
                name: "pulse".into(),
                duration_ms: 100.0,
                fill_mode: AnimationFillMode::None,
                ..Animation::default()
            },
            0.0,
        ),
    ];

    assert!(sample_animation_runs(&runs, &sheet.keyframes, 150.0, &BTreeMap::new()).is_empty());
}

#[test]
fn controller_preserves_playback_across_updates_and_drops_removed_nodes() {
    use lunelle_core::NodeId;

    fn tree(nodes: &[u64], play_state: &str) -> ComputedStyleTree {
        ComputedStyleTree {
            snapshot_version: 1,
            stylesheet_version: 1,
            environment_version: 1,
            styles: nodes
                .iter()
                .map(|id| ComputedStyle {
                    node: NodeId::from_raw(*id),
                    properties: BTreeMap::from([
                        ("animation-name".into(), "fade".into()),
                        ("animation-duration".into(), "1000ms".into()),
                        ("animation-timing-function".into(), "linear".into()),
                        ("animation-fill-mode".into(), "both".into()),
                        ("animation-play-state".into(), play_state.into()),
                        ("opacity".into(), "1".into()),
                    ]),
                })
                .collect(),
            diagnostics: Vec::new(),
        }
    }

    let sheet = StyleSheet::parse("@keyframes fade { from { opacity: 0; } to { opacity: 1; } }");
    let mut controller = AnimationController::default();
    assert_eq!(
        controller
            .update(tree(&[1, 2], "running"), &sheet.keyframes, 0.0)
            .styles[0]
            .properties["opacity"],
        "0"
    );
    assert_eq!(
        controller
            .update(tree(&[1, 2], "running"), &sheet.keyframes, 500.0)
            .styles[0]
            .properties["opacity"],
        "0.5"
    );
    assert_eq!(
        controller
            .update(tree(&[1, 2], "paused"), &sheet.keyframes, 600.0)
            .styles[0]
            .properties["opacity"],
        "0.6"
    );
    assert_eq!(
        controller
            .update(tree(&[1, 2], "paused"), &sheet.keyframes, 900.0)
            .styles[0]
            .properties["opacity"],
        "0.6"
    );
    assert_eq!(
        controller
            .update(tree(&[1, 2], "running"), &sheet.keyframes, 900.0)
            .styles[0]
            .properties["opacity"],
        "0.6"
    );
    assert_eq!(
        controller
            .update(tree(&[1], "running"), &sheet.keyframes, 1100.0)
            .styles[0]
            .properties["opacity"],
        "0.8"
    );
    assert_eq!(controller.run_count(), 1);
}

#[test]
fn controller_requests_frames_only_while_timelines_are_running() {
    use lunelle_core::NodeId;

    let make_tree = |iteration_count: &str| ComputedStyleTree {
        snapshot_version: 1,
        stylesheet_version: 1,
        environment_version: 1,
        styles: vec![ComputedStyle {
            node: NodeId::from_raw(1),
            properties: BTreeMap::from([
                ("animation-name".into(), "fade".into()),
                ("animation-duration".into(), "100ms".into()),
                ("animation-iteration-count".into(), iteration_count.into()),
            ]),
        }],
        diagnostics: Vec::new(),
    };
    let sheet = StyleSheet::parse("@keyframes fade { from { opacity: 0; } to { opacity: 1; } }");
    let mut finite = AnimationController::default();
    finite.update(make_tree("1"), &sheet.keyframes, 0.0);
    assert!(finite.needs_frame(50.0));
    assert!(!finite.needs_frame(100.0));

    let mut infinite = AnimationController::default();
    infinite.update(make_tree("infinite"), &sheet.keyframes, 0.0);
    assert!(infinite.needs_frame(10_000.0));
}
