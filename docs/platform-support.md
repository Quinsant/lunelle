# Platform Support

| Platform | Target | Build requirements | Runtime requirements |
| --- | --- | --- | --- |
| Windows | Windows 10+ | Rust stable and supported Windows toolchain | Window system and a GPU driver/backend supported by wgpu |
| macOS | macOS 12+ | Rust stable and supported Apple toolchain | Window system and Metal-capable GPU |
| Linux | X11 or Wayland | Rust stable, X11/Wayland development headers | Supported display server and GPU driver/backend (Vulkan or GLES as available) |

The current root application uses `winit 0.30` and `wgpu 24`. GPU API availability is selected by `wgpu` at runtime and depends on installed drivers (Vulkan, Metal, DX12, or GLES backends supported by the selected build).

CI runs the workspace tests and compiles all targets/features on Linux, Windows, and macOS. These checks do not create a native window, initialize a GPU adapter, or certify runtime rendering; those platform integration checks remain outstanding.

The callback-driven application loop is the primary desktop path. The separately exposed polling backend uses winit's platform-specific event-pumping API and should be validated on each target before relying on it. Native window creation must run on the platform's event-loop thread. Fonts and GPU capabilities depend on the host system, so rendering may differ between installations.
