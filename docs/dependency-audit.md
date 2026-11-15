# Dependency Audit

Checked 2026-11-08 against the crates.io crate pages. `Cargo.lock` is committed for reproducible builds; listed manifest constraints select compatible release lines, not necessarily each registry's newest major release.

| Dependency | Locked version | Registry version checked | License noted by registry | Decision |
| --- | --- | --- | --- | --- |
| `winit` | 0.30.13 | 0.30.13 | Apache-2.0 | Current compatible 0.30 line |
| `wgpu` | 24.0.5 | 30.0.1 | MIT OR Apache-2.0 | Keep 24 pending renderer API migration and GPU validation |
| `glyphon` | 0.8.0 | 0.12.0 | MIT OR Apache-2.0 OR Zlib | Keep the wgpu-24-compatible line; newer API alignment is not validated |
| `cosmic-text` | 0.12.1 | 0.19.0 | MIT OR Apache-2.0 | Exact pin retained with current glyphon integration |
| `cssparser` | 0.38.0 | 0.38.0 | MPL-2.0 | Updated; CSS parser tests pass on the current API |
| `crossbeam-channel` | 0.5.17 | 0.5.17 | MIT OR Apache-2.0 | Current compatible release |
| `thiserror` | 2.0.21 | 2.0.21 | MIT OR Apache-2.0 | Current compatible release |
| `syn` | 3.0.9 (direct); 2.0.119 (transitive) | 3.0.9 | MIT OR Apache-2.0 | Macros use syn 3; syn 2 remains required by a transitive thiserror 1 dependency |
| `quote` | 1.0.47 | 1.0.47 | MIT OR Apache-2.0 | Current compatible release |
| `proc-macro2` | 1.0.107 | 1.0.107 | MIT OR Apache-2.0 | Current compatible release |
| `image` | 0.25.10 | 0.25.10 | MIT OR Apache-2.0 | Updated to current patch release; PNG/JPEG features remain explicit |
| `pollster` | 1.0.1 | 1.0.1 | Apache-2.0 OR MIT | Updated; all-feature GPU initialization path compiles |

The wgpu 30, glyphon 0.12, and cosmic-text 0.19 lines are not treated as drop-in updates. The renderer and text stack need dedicated compatibility checks and, where APIs differ, migrations before adopting them. The proc-macro crate has been migrated to syn 3; syn 2 remains in the lockfile because a transitive thiserror 1 dependency requires it. Current cargo metadata reports MPL-2.0 for `cssparser`; downstream redistribution should include the corresponding license notice. Other rows are reported as dual or triple licensed by their registry pages. This snapshot records selected direct dependencies only: transitive-license inventory, per-target platform compatibility, and MSRV certification remain incomplete. It is not a legal opinion.

Verification record:

```sh
cargo test --workspace --locked --no-default-features
cargo check --workspace --locked --all-targets --all-features
```

After the `image` patch update, both commands passed locally on Linux. Following the `cssparser` update, the focused `cargo test -p lunelle-css --locked` passed. After updating `pollster`, all-target/all-feature check passed. After updating direct `syn` to 3, `cargo test --workspace --locked --no-default-features` passed (177 root tests, all crate and integration tests, and compile-fail doctests), as did `cargo check --workspace --locked --all-targets --all-features`. Windows/macOS matrix execution is delegated to CI; no native window/GPU runtime certification is implied.

Sources: [winit](https://crates.io/crates/winit), [wgpu](https://crates.io/crates/wgpu), [glyphon](https://crates.io/crates/glyphon), [cosmic-text](https://crates.io/crates/cosmic-text), [cssparser](https://crates.io/crates/cssparser), [crossbeam-channel](https://crates.io/crates/crossbeam-channel), [thiserror](https://crates.io/crates/thiserror), [syn](https://crates.io/crates/syn), [quote](https://crates.io/crates/quote), [proc-macro2](https://crates.io/crates/proc-macro2), [image](https://crates.io/crates/image), [pollster](https://crates.io/crates/pollster).