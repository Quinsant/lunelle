use lunelle_css::{
    StepPosition, TimingFunction, Transition, TransitionProperty, TransitionRun, TransitionSample,
    parse_transition,
};

#[test]
fn parses_transition_lists_times_and_timing_functions() {
    let transitions = parse_transition("opacity 250ms ease-in 0.1s, transform 1s cubic-bezier(0.2, 0.4, 0.8, 1) -50ms, color 3s steps(4, jump-start)").unwrap();
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].duration_ms, 250.0);
    assert_eq!(transitions[0].delay_ms, 100.0);
    assert_eq!(transitions[0].timing_function, TimingFunction::EaseIn);
    assert_eq!(
        transitions[1].timing_function,
        TimingFunction::CubicBezier(0.2, 0.4, 0.8, 1.0)
    );
    assert_eq!(transitions[1].delay_ms, -50.0);
    assert_eq!(
        transitions[2].timing_function,
        TimingFunction::Steps(4, StepPosition::JumpStart)
    );
}

#[test]
fn applies_transition_defaults_and_property_keywords() {
    assert_eq!(
        parse_transition("opacity").unwrap()[0],
        Transition {
            property: TransitionProperty::Property("opacity".into()),
            ..Transition::default()
        }
    );
    assert_eq!(
        parse_transition("none").unwrap()[0].property,
        TransitionProperty::None
    );
    assert_eq!(
        parse_transition("none 1s linear").unwrap()[0].duration_ms,
        1000.0
    );
    assert_eq!(
        parse_transition("all 0s linear").unwrap()[0].property,
        TransitionProperty::All
    );
}

#[test]
fn normalizes_transition_property_names() {
    assert_eq!(
        parse_transition("BACKGROUND-COLOR 1s").unwrap()[0].property,
        TransitionProperty::Property("background-color".into())
    );
}

#[test]
fn rejects_invalid_times_and_timing_arguments() {
    for value in [
        "opacity -1s",
        "opacity 1s 2s 3s",
        "opacity 1px",
        "opacity cubic-bezier(-0.1, 0, 0.5, 1)",
        "opacity steps(1, jump-none)",
        "opacity 1s,",
    ] {
        assert!(parse_transition(value).is_none(), "accepted {value}");
    }
}

#[test]
fn timeline_honors_delay_interpolates_and_finishes() {
    let run = TransitionRun::new(
        0.0,
        100.0,
        10.0,
        Transition {
            duration_ms: 200.0,
            delay_ms: 50.0,
            timing_function: TimingFunction::Linear,
            ..Transition::default()
        },
    );
    assert_eq!(
        run.sample(40.0),
        TransitionSample {
            value: 0.0,
            finished: false
        }
    );
    assert_eq!(
        run.sample(160.0),
        TransitionSample {
            value: 50.0,
            finished: false
        }
    );
    assert_eq!(
        run.sample(260.0),
        TransitionSample {
            value: 100.0,
            finished: true
        }
    );
}

#[test]
fn retargeting_starts_from_the_current_value_and_timing_functions_are_sampled() {
    let mut run = TransitionRun::new(
        0.0,
        100.0,
        0.0,
        Transition {
            duration_ms: 100.0,
            timing_function: TimingFunction::Linear,
            ..Transition::default()
        },
    );
    run.retarget(
        40.0,
        0.0,
        Transition {
            duration_ms: 60.0,
            timing_function: TimingFunction::Linear,
            ..Transition::default()
        },
    );
    assert_eq!(run.sample(40.0).value, 40.0);
    assert_eq!(run.sample(70.0).value, 20.0);
    assert_eq!(
        TimingFunction::Steps(4, StepPosition::JumpEnd).sample(0.3),
        0.25
    );
    assert!(TimingFunction::Ease.sample(0.5) > 0.5);
    assert_eq!(TimingFunction::Linear.sample(0.5), 0.5);
}
