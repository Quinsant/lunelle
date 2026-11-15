use crate::Element;
use std::collections::HashMap;

pub(crate) fn retain_matching_ids(previous: &dyn Element, next: &mut dyn Element) {
    if previous.tag_name() == next.tag_name() {
        next.set_id(previous.id());
        let previous_children = previous.children();
        let mut matched_previous = vec![false; previous_children.len()];
        let mut keyed_previous = HashMap::<&str, Vec<usize>>::new();
        for (index, child) in previous_children.iter().enumerate() {
            if let Some(key) = child.element_id() {
                keyed_previous.entry(key).or_default().push(index);
            }
        }
        for (next_index, new_child) in next.children_mut().iter_mut().enumerate() {
            let previous_index = new_child
                .element_id()
                .and_then(|key| {
                    keyed_previous.get(key).and_then(|indices| {
                        indices.iter().copied().find(|&index| {
                            !matched_previous[index]
                                && previous_children[index].tag_name() == new_child.tag_name()
                        })
                    })
                })
                .or_else(|| {
                    previous_children.get(next_index).and_then(|old_child| {
                        (!matched_previous[next_index]
                            && old_child.tag_name() == new_child.tag_name()
                            && old_child.element_id().is_none()
                            && new_child.element_id().is_none())
                        .then_some(next_index)
                    })
                });
            if let Some(previous_index) = previous_index {
                matched_previous[previous_index] = true;
                retain_matching_ids(
                    previous_children[previous_index].as_ref(),
                    new_child.as_mut(),
                );
            }
        }
    }
}
