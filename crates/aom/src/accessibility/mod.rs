use crate::Element;

/// Read-only accessibility navigation view borrowed during synchronous UI-thread use.
///
/// Implementations are not required to be `Send` or `Sync`; callers retain ownership
/// of the tree and expose only borrowed node data and child slices.
pub trait FocusableTree {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessibilityRole {
    #[default]
    Generic,
    Button,
    TextInput,
    Heading,
    Image,
    List,
    ListItem,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accessibility {
    pub role: AccessibilityRole,
    pub label: Option<String>,
    pub focusable: bool,
    pub disabled: bool,
}

#[derive(Default)]
pub struct FocusManager {
    focusable_ids: Vec<String>,
    focused: Option<usize>,
}

impl FocusManager {
    pub fn rebuild(&mut self, root: &dyn Element) {
        let focused_id = self.current().map(str::to_owned);
        self.focusable_ids.clear();
        collect_focusable(root, &mut self.focusable_ids);
        self.focused = focused_id.and_then(|id| {
            self.focusable_ids
                .iter()
                .position(|candidate| candidate == &id)
        });
    }

    pub fn focus(&mut self, id: &str) -> Option<&str> {
        self.focused = self
            .focusable_ids
            .iter()
            .position(|candidate| candidate == id);
        self.current()
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    pub fn focus_next(&mut self) -> Option<&str> {
        if self.focusable_ids.is_empty() {
            self.focused = None;
            return None;
        }
        self.focused = Some((self.focused.map_or(0, |index| index + 1)) % self.focusable_ids.len());
        self.current()
    }

    pub fn focus_previous(&mut self) -> Option<&str> {
        if self.focusable_ids.is_empty() {
            self.focused = None;
            return None;
        }
        let previous = self.focused.map_or(self.focusable_ids.len() - 1, |index| {
            if index == 0 {
                self.focusable_ids.len() - 1
            } else {
                index - 1
            }
        });
        self.focused = Some(previous);
        self.current()
    }

    pub fn current(&self) -> Option<&str> {
        self.focused
            .and_then(|index| self.focusable_ids.get(index))
            .map(String::as_str)
    }
}

fn collect_focusable(element: &dyn Element, ids: &mut Vec<String>) {
    if element.accessibility().focusable && !element.accessibility().disabled {
        if let Some(id) = element.element_id() {
            ids.push(id.to_owned());
        }
    }
    for child in element.children() {
        collect_focusable(child.as_ref(), ids);
    }
}
