#[derive(Clone, Copy, Debug)]
pub(super) enum StructuralPseudo {
    FirstChild,
    LastChild,
    OnlyChild,
    NthChild(i32, i32),
    NthLastChild(i32, i32),
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthOfType(i32, i32),
    NthLastOfType(i32, i32),
}

use super::SelectorNode;

pub(super) fn matches_structural(
    pseudo: StructuralPseudo,
    nodes: &[SelectorNode],
    index: usize,
) -> bool {
    let Some(node) = nodes.get(index) else {
        return false;
    };
    let Some(parent) = node.parent else {
        return false;
    };
    let same_type = matches!(
        pseudo,
        StructuralPseudo::FirstOfType
            | StructuralPseudo::LastOfType
            | StructuralPseudo::OnlyOfType
            | StructuralPseudo::NthOfType(..)
            | StructuralPseudo::NthLastOfType(..)
    );
    let matches_sibling = |sibling: &&SelectorNode| {
        sibling.parent == Some(parent) && (!same_type || sibling.tag_name == node.tag_name)
    };
    let from_start = nodes[..index].iter().filter(matches_sibling).count() as i32 + 1;
    let from_end = nodes[index + 1..].iter().filter(matches_sibling).count() as i32 + 1;
    match pseudo {
        StructuralPseudo::FirstChild | StructuralPseudo::FirstOfType => from_start == 1,
        StructuralPseudo::LastChild | StructuralPseudo::LastOfType => from_end == 1,
        StructuralPseudo::OnlyChild | StructuralPseudo::OnlyOfType => {
            from_start == 1 && from_end == 1
        }
        StructuralPseudo::NthChild(a, b) | StructuralPseudo::NthOfType(a, b) => {
            nth_matches(a, b, from_start)
        }
        StructuralPseudo::NthLastChild(a, b) | StructuralPseudo::NthLastOfType(a, b) => {
            nth_matches(a, b, from_end)
        }
    }
}

fn nth_matches(a: i32, b: i32, position: i32) -> bool {
    if a == 0 {
        return position == b;
    }
    let difference = position - b;
    difference % a == 0 && difference / a >= 0
}

pub(super) fn parse_nth_formula(source: &str) -> Option<(i32, i32)> {
    let formula = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match formula.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let Some(n_index) = formula.find('n') else {
        return Some((0, formula.parse().ok()?));
    };
    if formula[n_index + 1..].contains('n') {
        return None;
    }
    let a = match &formula[..n_index] {
        "" | "+" => 1,
        "-" => -1,
        value => value.parse().ok()?,
    };
    let suffix = &formula[n_index + 1..];
    let b = if suffix.is_empty() {
        0
    } else {
        suffix.parse().ok()?
    };
    Some((a, b))
}
