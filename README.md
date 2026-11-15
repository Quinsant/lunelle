# Lunelle

An experimental Rust framework for building native interfaces with components, an HTML-like tree, and CSS. Applications open a native window using `winit`, calculate element layout, and render the interface with `wgpu`. This is not a browser or an implementation of the entire web platform.

> **This project is in an early stage of development.** The API and supported CSS profile may change; browser compatibility and production readiness are not guaranteed.

## Features

- Rust components with state, messages, events, lifecycle hooks, and element tree updates.
- `#[component]` and `view!` macros for describing components and interfaces.
- CSS styling with selectors, the cascade, inheritance, variables, Flexbox, Grid, typography, and limited animation support.
- Native windows, mouse and keyboard input, and GPU rendering of text and elements.
- Accessibility semantics and AccessKit integration; native provider support still needs verification on target platforms.
- Background tasks and resource loading with results delivered to the UI thread.

CSS support is partial, and individual properties may be supported differently during parsing, layout, and rendering. See the [CSS guide](docs/css-profile.md) and [AOM guide](docs/components.md) for details and limitations.

## Getting Started

Install stable Rust with support for edition 2024. Running a window requires a graphical session and the system libraries and drivers supported by `winit` and `wgpu`. On Linux, X11 or Wayland and an available Vulkan or GLES graphics backend are used.

Clone the repository, change into its directory, and run the example:

```sh
cargo run --example counter
```

The [counter](examples/counter.rs) opens a window where buttons change component state and an embedded CSS stylesheet controls the appearance. The example uses the `base_component` feature, which is enabled by default.

An interface fragment from the example:

```rust
view! {
    <div class="actions">
        <button id="decrement" class="counter-button">"- 1"</button>
        <button id="increment" class="counter-button">"+ 1"</button>
    </div>
}
```

The complete example shows event handling, state, and window creation.

## Project Structure

The project is organized as a Cargo workspace:

| Crate | Purpose |
| --- | --- |
| `lunelle-core` | Windows, events, resources, and runtime. |
| `lunelle-aom` | Element tree, components, events, and accessibility semantics. |
| `lunelle-css` | CSS parsing, cascade, style computation, and layout and rendering support. |
| `lunelle-macros` | Component and view macros. |
| `lunelle-application` | Integration of components, styles, windows, and the renderer. |

The root crate, `lunelle`, exposes the application API and re-exports the subsystems. The package name is written as it appears in `Cargo.toml`.

## Documentation

- [CSS: features, limitations, and examples](docs/css-profile.md)
- [AOM: components, events, and examples](docs/components.md)
- [Architecture](docs/architecture.md)
- [Platform support](docs/platform-support.md)