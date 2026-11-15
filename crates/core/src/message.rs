use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};

/// Creates a multi-producer, multi-consumer channel for messages shared across threads.
pub struct MessageQueue;

pub struct MessageSender<T>(Sender<T>);

pub struct MessageReceiver<T>(Receiver<T>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryReceiveError {
    Empty,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageClosed;

impl MessageQueue {
    /// Creates a channel that can hold at most `capacity` pending messages.
    pub fn bounded<T>(capacity: usize) -> (MessageSender<T>, MessageReceiver<T>) {
        let (sender, receiver) = bounded(capacity);
        (MessageSender(sender), MessageReceiver(receiver))
    }

    /// Creates a channel with no fixed message capacity.
    pub fn unbounded<T>() -> (MessageSender<T>, MessageReceiver<T>) {
        let (sender, receiver) = unbounded();
        (MessageSender(sender), MessageReceiver(receiver))
    }
}

impl<T> Clone for MessageSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for MessageReceiver<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> MessageSender<T> {
    /// Sends a message, returning ownership of it if all receivers were dropped.
    pub fn send(&self, message: T) -> Result<(), T> {
        self.0.send(message).map_err(|error| error.0)
    }
}

impl<T> MessageReceiver<T> {
    /// Waits until a message arrives or all senders are dropped.
    pub fn recv(&self) -> Result<T, MessageClosed> {
        self.0.recv().map_err(|_| MessageClosed)
    }

    /// Receives a message without waiting.
    pub fn try_recv(&self) -> Result<T, TryReceiveError> {
        self.0.try_recv().map_err(|error| match error {
            TryRecvError::Empty => TryReceiveError::Empty,
            TryRecvError::Disconnected => TryReceiveError::Disconnected,
        })
    }
}
