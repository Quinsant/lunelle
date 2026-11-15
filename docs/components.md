# AOM: Components and the Interface Tree

`lunelle-aom` provides the Application Object Model: an element tree, stateful components, event dispatch, and accessibility data. AOM separates the interface description from CSS, the windowing system, and GPU rendering.

## Features

- The `Element` trait describes tree elements; built-in `Div`, `Button`, `P`, `Text`, `Input`, `List`, and `Li` are available with the `base_component` feature.
- The `Component` trait defines input and message types, state creation and updates, a view, and `on_mount` / `on_unmount` hooks.
- `ComponentInstance` owns a component and its tree; after a message, it rebuilds the view and preserves matching element identities.
- Events are dispatched through capture, target, and bubble phases; mouse data and other event types are available.
- Semantic roles, labels, disabled state, and focus management are used to build an accessibility tree.
- Immutable tree snapshots are passed to the CSS engine; versions allow stale background computation results to be rejected.
- The `#[component]` and `view!` macros provide declarative component and element descriptions.

`base_component` is enabled by default in the root crate. When default features are disabled, enable the built-in elements explicitly or provide your own `Element` implementation.

## Component Example

A component can be implemented directly, without macros:

```rust
use lunelle_aom::{Component, ComponentInstance, Element, P};

struct Counter(usize);

impl Component for Counter {
    type Props = usize;
    type Message = usize;

    fn create(initial: usize) -> Self {
        Self(initial)
    }

    fn update(&mut self, increment: usize) {
        self.0 += increment;
    }

    fn view(&self) -> Box<dyn Element> {
        Box::new(P::new().with_text(self.0.to_string()))
    }
}

let mut counter = ComponentInstance::<Counter>::new(2);
counter.dispatch(3);
assert_eq!(counter.element().text(), Some("5"));
```

This example works with an in-memory tree and does not open a window. For a complete example using macros, CSS, and click handling, see [counter.rs](../examples/counter.rs).

## Snapshots and Identity

`element.style_snapshot(version)` creates a snapshot for CSS computation. Check the result version before applying it; when using a context, also check the stylesheet and environment versions.

When a view is rebuilt, children with matching tags and `with_id` keys retain their internal identities when reordered. Unkeyed elements are matched by position and tag. Keys are local to a parent's child list.

## Limitations

- Components and their hooks run synchronously on the UI owner thread; background tasks should use owned data and send their results back to the UI thread.
- Updating an instance rebuilds the view and then reconciles its elements; this does not guarantee fully incremental updates across all application subsystems.
- Use unique IDs within the relevant tree to address interactive elements.
- AccessKit publishes the semantic tree to the native platform, but system provider behavior still requires verification on target operating systems.
- The macros and built-in elements do not implement all of HTML, and CSS supports only the documented [profile](css-profile.md).