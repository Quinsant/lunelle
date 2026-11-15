use lunelle_aom::{Component, ComponentHost, ComponentInstance};
use lunelle_core::{
    MessageQueue, MessageReceiver, MessageSender, ResourceKind, ResourceTable, Runtime,
    WindowOptions,
};
use lunelle_css::StyleSheet;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::RunError;

pub struct Application {
    context: ApplicationContext,
}

/// Shared application services available to the application callback.
pub struct ApplicationContext {
    runtime: Arc<Runtime>,
    state: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    resources: ResourceTable,
    messages: (
        MessageSender<ApplicationMessage>,
        MessageReceiver<ApplicationMessage>,
    ),
    windows: Mutex<(Option<WindowRequest>, bool)>,
}

/// Untyped application message suitable for communication between worker tasks and UI code.
pub struct ApplicationMessage(Box<dyn Any + Send>);

impl ApplicationMessage {
    pub fn new<T: Send + 'static>(value: T) -> Self {
        Self(Box::new(value))
    }

    pub fn downcast<T: 'static>(self) -> Result<T, Self> {
        self.0.downcast().map(|value| *value).map_err(Self)
    }
}

pub(crate) struct WindowRequest {
    pub(crate) options: WindowOptions,
    pub(crate) root: Box<dyn FnOnce() -> Box<dyn ComponentHost>>,
}

pub struct ApplicationBuilder {
    resources: ResourceTable,
    state: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    threads: usize,
}

impl Application {
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder {
            resources: ResourceTable::new(),
            state: HashMap::new(),
            threads: std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(1),
        }
    }

    pub fn context(&self) -> &ApplicationContext {
        &self.context
    }

    pub fn run<F>(self, callback: F) -> Result<(), RunError>
    where
        F: FnOnce(&ApplicationContext),
    {
        super::windowed::run(self, callback)
    }
}

impl ApplicationBuilder {
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    pub fn resource(mut self, resources: ResourceTable) -> Self {
        self.resources = resources;
        self
    }

    pub fn state<T: Send + Sync + 'static>(mut self, state: T) -> Self {
        self.state
            .insert(TypeId::of::<T>(), Arc::new(Mutex::new(state)));
        self
    }

    pub fn build(self) -> Application {
        let runtime = Arc::new(Runtime::new(self.threads));
        let messages = MessageQueue::unbounded();
        Application {
            context: ApplicationContext {
                runtime,
                state: self.state,
                resources: self.resources,
                messages,
                windows: Mutex::new((None, false)),
            },
        }
    }
}

impl ApplicationContext {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub(crate) fn runtime_handle(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    pub fn send<T: Send + 'static>(&self, message: T) -> Result<(), T> {
        match self.messages.0.send(ApplicationMessage::new(message)) {
            Ok(()) => Ok(()),
            Err(message) => Err(*message
                .0
                .downcast()
                .expect("application message type changed")),
        }
    }

    pub fn try_receive(&self) -> Result<ApplicationMessage, lunelle_core::TryReceiveError> {
        self.messages.1.try_recv()
    }

    pub fn state<T: Send + Sync + 'static>(&self) -> Option<Arc<Mutex<T>>> {
        self.state.get(&TypeId::of::<T>())?.clone().downcast().ok()
    }

    pub fn stylesheets(&self) -> Result<Vec<StyleSheet>, crate::RunError> {
        self.resources
            .resources()
            .iter()
            .filter(|resource| resource.kind == ResourceKind::Css)
            .map(|resource| {
                std::str::from_utf8(&resource.bytes)
                    .map(StyleSheet::parse)
                    .map_err(|error| {
                        crate::RunError::Application(format!(
                            "CSS resource {:?} is not valid UTF-8: {error}",
                            resource.path
                        ))
                    })
            })
            .collect()
    }

    pub fn resources(&self) -> &ResourceTable {
        &self.resources
    }

    pub fn spawn_window<C: Component + 'static>(
        &self,
        props: C::Props,
        options: WindowOptions,
    ) -> Result<(), WindowSpawnError> {
        let mut windows = self
            .windows
            .lock()
            .map_err(|_| WindowSpawnError::Poisoned)?;
        if windows.0.is_some() || windows.1 {
            return Err(WindowSpawnError::MultipleWindowsUnsupported);
        }
        windows.0 = Some(WindowRequest {
            options,
            root: Box::new(move || Box::new(ComponentInstance::<C>::new(props))),
        });
        Ok(())
    }

    pub(crate) fn take_window(&self) -> Result<Option<WindowRequest>, WindowSpawnError> {
        let mut windows = self
            .windows
            .lock()
            .map_err(|_| WindowSpawnError::Poisoned)?;
        windows.1 = true;
        Ok(windows.0.take())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowSpawnError {
    #[error("only one window is supported")]
    MultipleWindowsUnsupported,
    #[error("application context lock was poisoned")]
    Poisoned,
}
