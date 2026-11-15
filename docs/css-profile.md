# CSS: Features and Limitations

`lunelle-css` parses stylesheets, matches selectors against the AOM tree, and computes styles for layout and rendering. It is a limited profile for native interfaces, not a browser CSS engine. Support for syntax does not imply that it is applied consistently at every stage of rendering.

## Features

- Element, class, and ID selectors, compound selectors, selector lists, and descendant and child combinators.
- Cascade based on specificity, declaration order, and `!important`; text property inheritance, `inherit`, `initial`, and `unset`.
- Inherited custom properties (`--name`) and `var()` references with fallback values.
- Block, Flexbox, and Grid layout, sizing, spacing, positioning, and `overflow`.
- Colors, backgrounds, borders, text, and fonts; media conditions for viewport dimensions and resolution.
- Parsing of two-dimensional transforms, transitions, and `@keyframes`; application support depends on the property and renderer.
- Diagnostics for invalid and unsupported constructs, and versioned style computation from a tree snapshot.

## Styling Example

The following rules match classes and IDs on AOM elements:

```css
.counter-panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 32px;
    background-color: #ffffff;
}

.counter-button {
    color: #ffffff;
    background-color: #0f766e;
    border-radius: 6px;
}

.counter-button:hover {
    opacity: 0.8;
}
```

The [counter example](../examples/counter.rs) registers CSS as a resource and opens a window:

```rust
let mut resources = ResourceTable::new();
resources.add("counter.css", ResourceKind::Css, styles.as_bytes().to_vec());

Application::builder()
    .resource(resources)
    .build();
```

To compute styles separately, use an AOM snapshot and `CssRender`:

```rust
use lunelle_aom::{Div, Element};
use lunelle_css::CssRender;

let root = Div::new().with_class("theme");
let snapshot = root.style_snapshot(1);
let css = CssRender::parse(".theme { color: navy; }");
let (result, diagnostics) = snapshot.compute(&css);

assert!(diagnostics.is_empty());
assert!(snapshot.accepts_result(result.version).is_ok());
```

For asynchronous computation with a context, also check the stylesheet and environment versions with `accepts_computed_result`: stale results must not be applied to the current tree.

## Limitations

- Full CSS support and browser compatibility are not claimed. Consult the implementation and crate tests for supported values and diagnostics.
- Grid implements only part of the CSS algorithm, including limited intrinsic sizing; `minmax()` and automatic placement also have restrictions.
- Media queries support a limited set of conditions, not the full CSS Media Queries feature set.
- Transforms are not yet applied consistently across text, geometry, clipping, and hit testing.
- Native transitions are not integrated for every property; transition lifecycle events are not implemented. Animation and rendering support only a subset of CSS properties.
- Scrolling and clipping are limited by the current renderer and should not be expected to match full browser behavior.
- Dynamic stylesheet replacement and arbitrary tree updates are not fully connected to the native style computation path.

See also the [AOM guide](components.md) and [complete example](../examples/counter.rs).