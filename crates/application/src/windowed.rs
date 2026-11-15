mod aom_css;
mod gpu;
mod interaction;
mod renderer;
mod window;

use lunelle_aom::{ComponentHost, UiAction};
use lunelle_core::{
    Runtime, WindowEvent,
    window_winit::{ApplicationControl, WindowApplication},
};
use lunelle_css::StyleSheet;
use std::sync::{Arc, mpsc};
use thiserror::Error;
use winit::window::Window;

#[derive(Debug, Error)]
pub enum RunError {
    #[error("application setup failed: {0}")]
    Application(String),
    #[error("native window failed: {0}")]
    Window(#[from] lunelle_core::window_winit::WindowError),
    #[error("native renderer failed: {0}")]
    Renderer(String),
}

pub(super) fn run<F>(application: crate::Application, callback: F) -> Result<(), RunError>
where
    F: FnOnce(&crate::ApplicationContext),
{
    callback(application.context());
    let request = application
        .context()
        .take_window()
        .map_err(|error| RunError::Application(error.to_string()))?
        .ok_or_else(|| RunError::Application("application did not spawn a window".into()))?;
    let stylesheets = application.context().stylesheets()?;
    let host = NativeHost::new(
        (request.root)(),
        stylesheets,
        application.context().runtime_handle(),
    );
    let host = window::run(request.options, host)?;
    host.finish()
}

struct NativeHost {
    renderer: Option<renderer::Renderer>,
    pending_window: Option<Arc<Window>>,
    renderer_receiver: Option<mpsc::Receiver<Result<renderer::Renderer, String>>>,
    renderer_thread: Option<std::thread::JoinHandle<()>>,
    component: Box<dyn ComponentHost>,
    stylesheets: Vec<StyleSheet>,
    runtime: Arc<Runtime>,
    error: Option<String>,
}

impl NativeHost {
    fn new(
        component: Box<dyn ComponentHost>,
        stylesheets: Vec<StyleSheet>,
        runtime: Arc<Runtime>,
    ) -> Self {
        Self {
            renderer: None,
            pending_window: None,
            renderer_receiver: None,
            renderer_thread: None,
            component,
            stylesheets,
            runtime,
            error: None,
        }
    }

    fn dispatch_actions(&mut self, actions: Vec<UiAction>) {
        if actions.is_empty() {
            return;
        }
        for action in actions {
            self.component.dispatch_action(action);
        }
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_element(self.component.element());
        }
    }

    fn finish(self) -> Result<(), RunError> {
        self.error
            .map_or(Ok(()), |error| Err(RunError::Renderer(error)))
    }
}

impl WindowApplication for NativeHost {
    fn exiting(&mut self) {
        self.runtime.set_ui_waker(None);
        // GPU resources must be dropped while the window event loop is still alive.
        if let Some(thread) = self.renderer_thread.take() {
            let _ = thread.join();
        }
        if let Some(receiver) = self.renderer_receiver.take() {
            if let Ok(Ok(renderer)) = receiver.try_recv() {
                self.renderer = Some(renderer);
            }
        }
        if let Some(mut renderer) = self.renderer.take() {
            renderer.shutdown();
        }
        self.pending_window = None;
    }

    fn resumed(&mut self, window: Arc<Window>) -> bool {
        let wake_window = window.clone();
        self.runtime
            .set_ui_waker(Some(Arc::new(move || wake_window.request_redraw())));
        window.request_redraw();
        self.pending_window = Some(window);
        false
    }

    fn about_to_wait(&mut self) -> ApplicationControl {
        if let Some(receiver) = &self.renderer_receiver {
            match receiver.try_recv() {
                Ok(Ok(renderer)) => {
                    renderer.request_redraw();
                    self.renderer = Some(renderer);
                    self.renderer_receiver = None;
                }
                Ok(Err(error)) => {
                    self.error = Some(error);
                    return ApplicationControl::Exit;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("renderer initialization thread stopped unexpectedly".into());
                    return ApplicationControl::Exit;
                }
                Err(mpsc::TryRecvError::Empty) => return ApplicationControl::Wait,
            }
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return ApplicationControl::Wait;
        };
        renderer.flush_interaction();
        if let Err(error) = renderer.poll_css_initialization() {
            self.error = Some(error.to_string());
            return ApplicationControl::Exit;
        }
        renderer.start_css_initialization();
        if let Err(error) = self.runtime.process_ui_tasks() {
            self.error = Some(error.to_string());
            return ApplicationControl::Exit;
        }
        match renderer.advance() {
            Ok(true) => ApplicationControl::Redraw,
            Ok(false) => ApplicationControl::Wait,
            Err(error) => {
                self.error = Some(error.to_string());
                ApplicationControl::Exit
            }
        }
    }

    fn window_event(&mut self, event: WindowEvent) -> bool {
        if self.renderer.is_none() {
            return match event {
                WindowEvent::CloseRequested => true,
                WindowEvent::RedrawRequested => {
                    if self.renderer_receiver.is_none() {
                        let Some(window) = self.pending_window.take() else {
                            return true;
                        };
                        let root = self.component.element().clone_box();
                        let stylesheets = self.stylesheets.clone();
                        let runtime = self.runtime.clone();
                        let (sender, receiver) = mpsc::channel();
                        let redraw_window = window.clone();
                        self.renderer_thread = Some(std::thread::spawn(move || {
                            let result = pollster::block_on(renderer::Renderer::new(
                                window,
                                root.as_ref(),
                                &stylesheets,
                                runtime,
                            ))
                            .map_err(|error| error.to_string());
                            let _ = sender.send(result);
                            redraw_window.request_redraw();
                        }));
                        self.renderer_receiver = Some(receiver);
                    }
                    false
                }
                _ => false,
            };
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return true;
        };
        match event {
            WindowEvent::CloseRequested => true,
            WindowEvent::Resized(size) => {
                renderer.resize_logical(size);
                false
            }
            WindowEvent::ScaleFactorChanged(_) => {
                renderer.scale_factor_changed();
                false
            }
            WindowEvent::Focused(true) => {
                renderer.request_redraw();
                false
            }
            WindowEvent::PointerMoved(point) => {
                let actions = renderer.pointer_moved(point);
                self.dispatch_actions(actions);
                false
            }
            WindowEvent::PointerLeft => {
                let actions = renderer.pointer_left();
                self.dispatch_actions(actions);
                false
            }
            WindowEvent::PointerButton {
                button, pressed, ..
            } => {
                let action = renderer.pointer_button(pressed, button);
                self.dispatch_actions(action.into_iter().collect());
                false
            }
            WindowEvent::Scroll { delta } => {
                renderer.scroll(delta);
                false
            }
            WindowEvent::RedrawRequested => match renderer.render() {
                Ok(()) => false,
                Err(error) => {
                    self.error = Some(error.to_string());
                    true
                }
            },
            _ => false,
        }
    }
}
