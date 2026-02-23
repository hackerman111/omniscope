use std::time::Duration;
use anyhow::Result;

use crossterm::event::{Event as CrosstermEvent, KeyEvent};

/// Events that the TUI can handle.
#[derive(Debug)]
pub enum AppEvent {
    /// A key press event.
    Key(KeyEvent),
    /// Terminal was resized.
    Resize(u16, u16),
    /// Periodic tick for animations / background updates.
    Tick,
    /// Result of an asynchronous background task.
    AsyncResult(crate::app::async_tasks::AsyncResultType),
}

/// Receives terminal events and async task results from a background thread.
pub struct EventHandler {
    receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> (Self, tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let bg_tx = tx.clone();
        
        // Spawn a thread to handle standard crossterm events
        std::thread::spawn(move || {
            loop {
                if crossterm::event::poll(tick_rate).unwrap_or(false) {
                    if let Ok(event) = crossterm::event::read() {
                        let app_event = match event {
                            CrosstermEvent::Key(key) => AppEvent::Key(key),
                            CrosstermEvent::Resize(w, h) => AppEvent::Resize(w, h),
                            _ => continue,
                        };
                        if bg_tx.send(app_event).is_err() {
                            break;
                        }
                    }
                } else {
                    if bg_tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                }
            }
        });

        (Self { receiver: rx }, tx)
    }

    /// Block until the next event (key press, resize, tick, or async result).
    pub fn next(&mut self) -> Result<AppEvent> {
        self.receiver.blocking_recv().ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}
