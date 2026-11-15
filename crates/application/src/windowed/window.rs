use lunelle_core::{
    WindowOptions,
    window_winit::{WindowApplication, WindowError, run_application},
};

pub(super) fn run<A: WindowApplication + 'static>(
    options: WindowOptions,
    host: A,
) -> Result<A, WindowError> {
    run_application(options, host)
}
