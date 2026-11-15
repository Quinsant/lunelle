# Procedural Macro Development and Testing

The `lunelle-macros` crate uses `syn` to parse Rust input and the `view!` DSL, and `quote` to generate code. Generated code references public `lunelle_aom` APIs; keep the dependency direction one-way and do not depend on private runtime implementation details.

Keep parsing/expansion logic in ordinary functions so it can be tested directly. Unit tests in `src/lib.rs` cover successful expansion and diagnostics for unsupported attributes. Integration tests in `tests/` compile macro output against the public AOM API. Compile-fail doctests document invalid component signatures and ensure those cases remain rejected.

Run the focused suite with:

```sh
cargo test -p lunelle-macros --locked
```