use lunelle_css::{Selector, SelectorNode, Specificity};

fn node(tag: &str, parent: Option<usize>) -> SelectorNode {
    SelectorNode {
        tag_name: tag.into(),
        element_id: None,
        classes: Vec::new(),
        parent,
        disabled: false,
        focused: false,
        hovered: false,
        active: false,
    }
}

#[test]
fn selector_matches_child_and_descendant_chains() {
    let nodes = vec![
        node("main", None),
        node("section", Some(0)),
        node("button", Some(1)),
    ];
    let child = Selector::parse("main > section").unwrap();
    let descendant = Selector::parse("main button").unwrap();
    assert!(child.matches(&nodes, 1));
    assert!(descendant.matches(&nodes, 2));
    assert!(!child.matches(&nodes, 2));
}

#[test]
fn selector_matches_adjacent_and_general_siblings() {
    let nodes = vec![
        node("main", None),
        node("button", Some(0)),
        node("label", Some(0)),
        node("button", Some(0)),
    ];
    let adjacent = Selector::parse("button + label").unwrap();
    let general = Selector::parse("button ~ button").unwrap();
    assert!(adjacent.matches(&nodes, 2));
    assert!(general.matches(&nodes, 3));
    assert!(!adjacent.matches(&nodes, 3));
}

#[test]
fn sibling_combinators_allow_whitespace_around_operator() {
    let nodes = vec![
        node("main", None),
        node("button", Some(0)),
        node("label", Some(0)),
    ];
    for selector in [
        "button+label",
        "button +label",
        "button+ label",
        "button + label",
    ] {
        assert!(
            Selector::parse(selector).unwrap().matches(&nodes, 2),
            "{selector}"
        );
    }
}

#[test]
fn interaction_pseudo_classes_contribute_specificity_and_match_state() {
    let mut button = node("button", None);
    button.focused = true;
    let selector = Selector::parse("button.primary:focus").unwrap();
    button.classes.push("primary".into());
    assert_eq!(selector.specificity(), Specificity(0, 2, 1));
    assert!(selector.matches(&[button], 0));
}

#[test]
fn rejects_unsupported_and_incomplete_selectors() {
    assert!(Selector::parse("button:hover >").is_err());
    assert!(Selector::parse("button:unknown").is_err());
    assert!(Selector::parse("button[attr]").is_err());
}

#[test]
fn structural_pseudo_classes_match_sibling_positions() {
    let nodes = vec![
        node("main", None),
        node("button", Some(0)),
        node("label", Some(0)),
        node("button", Some(0)),
        node("aside", Some(0)),
        node("solo", None),
        node("leaf", Some(5)),
    ];
    assert!(
        Selector::parse("button:first-child")
            .unwrap()
            .matches(&nodes, 1)
    );
    assert!(Selector::parse(":last-child").unwrap().matches(&nodes, 4));
    assert!(
        Selector::parse("leaf:only-child")
            .unwrap()
            .matches(&nodes, 6)
    );
    assert!(
        Selector::parse(":nth-child(2n + 1)")
            .unwrap()
            .matches(&nodes, 3)
    );
    assert!(
        Selector::parse(":nth-last-child(2)")
            .unwrap()
            .matches(&nodes, 3)
    );
    assert!(
        !Selector::parse(":nth-child(even)")
            .unwrap()
            .matches(&nodes, 1)
    );
}

#[test]
fn of_type_pseudo_classes_count_only_same_type_siblings() {
    let nodes = vec![
        node("main", None),
        node("button", Some(0)),
        node("label", Some(0)),
        node("button", Some(0)),
        node("label", Some(0)),
    ];
    assert!(
        Selector::parse("button:first-of-type")
            .unwrap()
            .matches(&nodes, 1)
    );
    assert!(
        Selector::parse("button:last-of-type")
            .unwrap()
            .matches(&nodes, 3)
    );
    assert!(
        !Selector::parse("label:only-of-type")
            .unwrap()
            .matches(&nodes, 2)
    );
    assert!(
        Selector::parse("button:nth-of-type(2)")
            .unwrap()
            .matches(&nodes, 3)
    );
    assert!(
        Selector::parse("label:nth-last-of-type(2)")
            .unwrap()
            .matches(&nodes, 2)
    );
}

#[test]
fn structural_pseudo_classes_have_class_specificity() {
    assert_eq!(
        Selector::parse("button:nth-child(2n+1)")
            .unwrap()
            .specificity(),
        Specificity(0, 1, 1)
    );
    assert!(Selector::parse(":nth-child(nope)").is_err());
}
