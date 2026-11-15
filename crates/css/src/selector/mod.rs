mod structural;

use structural::{StructuralPseudo, matches_structural, parse_nth_formula};

/// Selector precedence tuple ordered by IDs, classes/pseudo-classes, and tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity(pub u32, pub u32, pub u32);

impl std::ops::Add for Specificity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
    AdjacentSibling,
    GeneralSibling,
}

#[derive(Clone, Debug)]
struct SelectorPart {
    selector: SimpleSelector,
    relation: Combinator,
}

#[derive(Clone, Debug, Default)]
struct SimpleSelector {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    disabled: bool,
    focused: bool,
    hovered: bool,
    active: bool,
    structural: Vec<StructuralPseudo>,
}

/// Node metadata and interactive state used while matching selectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorNode {
    pub tag_name: String,
    pub element_id: Option<String>,
    pub classes: Vec<String>,
    pub parent: Option<usize>,
    pub disabled: bool,
    pub focused: bool,
    pub hovered: bool,
    pub active: bool,
}

/// Parsed selector chain used by cascade matching.
#[derive(Clone, Debug)]
pub struct Selector {
    parts: Vec<SelectorPart>,
    specificity: Specificity,
}

impl SimpleSelector {
    fn specificity(&self) -> Specificity {
        Specificity(
            u32::from(self.id.is_some()),
            self.classes.len() as u32,
            u32::from(self.tag.is_some()),
        )
    }

    fn matches(&self, nodes: &[SelectorNode], index: usize) -> bool {
        let node = &nodes[index];
        self.tag.as_ref().is_none_or(|tag| tag == &node.tag_name)
            && self
                .id
                .as_ref()
                .is_none_or(|id| node.element_id.as_ref() == Some(id))
            && self
                .classes
                .iter()
                .all(|class| node.classes.contains(class))
            && (!self.disabled || node.disabled)
            && (!self.focused || node.focused)
            && (!self.hovered || node.hovered)
            && (!self.active || node.active)
            && self
                .structural
                .iter()
                .all(|pseudo| matches_structural(*pseudo, nodes, index))
    }
}

impl Selector {
    /// Parses the selector subset implemented by this crate.
    pub fn parse(selector: &str) -> Result<Self, ()> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut relation = Combinator::Descendant;
        let mut combinator_pending = false;
        let mut function_depth = 0usize;
        let flush = |current: &mut String,
                     relation,
                     parts: &mut Vec<SelectorPart>|
         -> Result<(), ()> {
            if current.is_empty() {
                return Ok(());
            }
            let characters: Vec<char> = current.chars().collect();
            let mut selector = SimpleSelector::default();
            let mut index = 0;
            while index < characters.len() {
                let marker = characters[index];
                if marker == '[' || marker == ']' {
                    return Err(());
                }
                let start = if marker == '#' || marker == '.' || marker == ':' {
                    index + 1
                } else {
                    index
                };
                if marker == '*' {
                    index += 1;
                    continue;
                }
                let mut end = start;
                while end < characters.len()
                    && (characters[end].is_ascii_alphanumeric()
                        || characters[end] == '-'
                        || characters[end] == '_')
                {
                    end += 1;
                }
                if end == start {
                    return Err(());
                }
                let value: String = characters[start..end].iter().collect();
                match marker {
                    '#' => {
                        if selector.id.replace(value).is_some() {
                            return Err(());
                        }
                    }
                    '.' => selector.classes.push(value),
                    ':' => match value.as_str() {
                        "disabled" if !selector.disabled => selector.disabled = true,
                        "focus" if !selector.focused => selector.focused = true,
                        "hover" if !selector.hovered => selector.hovered = true,
                        "active" if !selector.active => selector.active = true,
                        "first-child" => selector.structural.push(StructuralPseudo::FirstChild),
                        "last-child" => selector.structural.push(StructuralPseudo::LastChild),
                        "only-child" => selector.structural.push(StructuralPseudo::OnlyChild),
                        "first-of-type" => selector.structural.push(StructuralPseudo::FirstOfType),
                        "last-of-type" => selector.structural.push(StructuralPseudo::LastOfType),
                        "only-of-type" => selector.structural.push(StructuralPseudo::OnlyOfType),
                        "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                            let open = end;
                            if characters.get(open) != Some(&'(') {
                                return Err(());
                            }
                            let formula_start = open + 1;
                            let formula_end = characters[formula_start..]
                                .iter()
                                .position(|character| *character == ')')
                                .map(|offset| formula_start + offset)
                                .ok_or(())?;
                            let formula: String =
                                characters[formula_start..formula_end].iter().collect();
                            let (a, b) = parse_nth_formula(&formula).ok_or(())?;
                            selector.structural.push(match value.as_str() {
                                "nth-child" => StructuralPseudo::NthChild(a, b),
                                "nth-last-child" => StructuralPseudo::NthLastChild(a, b),
                                "nth-of-type" => StructuralPseudo::NthOfType(a, b),
                                _ => StructuralPseudo::NthLastOfType(a, b),
                            });
                            end = formula_end + 1;
                        }
                        _ => return Err(()),
                    },
                    _ if selector.tag.replace(value.to_ascii_lowercase()).is_some() => {
                        return Err(());
                    }
                    _ => {}
                }
                index = end;
            }
            parts.push(SelectorPart { selector, relation });
            current.clear();
            Ok(())
        };
        for character in selector.chars() {
            if function_depth > 0 {
                match character {
                    '(' => function_depth += 1,
                    ')' => function_depth -= 1,
                    _ => {}
                }
                current.push(character);
                continue;
            }
            match character {
                '(' => {
                    function_depth = 1;
                    current.push(character);
                }
                ')' => return Err(()),
                '>' | '+' | '~' => {
                    flush(&mut current, relation, &mut parts)?;
                    if parts.is_empty() || combinator_pending {
                        return Err(());
                    }
                    relation = match character {
                        '>' => Combinator::Child,
                        '+' => Combinator::AdjacentSibling,
                        '~' => Combinator::GeneralSibling,
                        _ => unreachable!(),
                    };
                    combinator_pending = true;
                }
                value if value.is_whitespace() => {
                    flush(&mut current, relation, &mut parts)?;
                    if !combinator_pending {
                        relation = Combinator::Descendant;
                    }
                }
                value => {
                    combinator_pending = false;
                    current.push(value);
                }
            }
        }
        if function_depth != 0 {
            return Err(());
        }
        flush(&mut current, relation, &mut parts)?;
        if parts.is_empty() || combinator_pending {
            return Err(());
        }
        let specificity = parts.iter().fold(Specificity::default(), |sum, part| {
            let local = part.selector.specificity();
            Specificity(
                sum.0 + local.0,
                sum.1
                    + local.1
                    + u32::from(part.selector.disabled)
                    + u32::from(part.selector.focused)
                    + u32::from(part.selector.hovered)
                    + u32::from(part.selector.active)
                    + part.selector.structural.len() as u32,
                sum.2 + local.2,
            )
        });
        Ok(Self { parts, specificity })
    }

    /// Returns the selector's cascade specificity.
    pub fn specificity(&self) -> Specificity {
        self.specificity
    }

    /// Matches the selector against a node at `index` in a flat node array.
    pub fn matches(&self, nodes: &[SelectorNode], index: usize) -> bool {
        fn match_at(
            chain: &[SelectorPart],
            part_index: usize,
            nodes: &[SelectorNode],
            node_index: usize,
        ) -> bool {
            let Some(node) = nodes.get(node_index) else {
                return false;
            };
            if !chain[part_index].selector.matches(nodes, node_index) {
                return false;
            }
            if part_index == 0 {
                return true;
            }
            match chain[part_index].relation {
                Combinator::Child => node
                    .parent
                    .is_some_and(|parent| match_at(chain, part_index - 1, nodes, parent)),
                Combinator::Descendant => {
                    let mut ancestor = node.parent;
                    while let Some(candidate) = ancestor {
                        if match_at(chain, part_index - 1, nodes, candidate) {
                            return true;
                        }
                        ancestor = nodes.get(candidate).and_then(|node| node.parent);
                    }
                    false
                }
                Combinator::AdjacentSibling | Combinator::GeneralSibling => {
                    let Some(parent) = node.parent else {
                        return false;
                    };
                    for candidate in (0..node_index).rev() {
                        if nodes[candidate].parent != Some(parent) {
                            continue;
                        }
                        if match_at(chain, part_index - 1, nodes, candidate) {
                            return true;
                        }
                        if chain[part_index].relation == Combinator::AdjacentSibling {
                            return false;
                        }
                    }
                    false
                }
            }
        }

        match_at(&self.parts, self.parts.len() - 1, nodes, index)
    }
}
