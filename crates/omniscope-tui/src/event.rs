use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

/// Events that the TUI can handle.
#[derive(Debug)]
pub enum AppEvent {
    /// A key press event.
    Key(KeyEvent),
    /// Terminal was resized.
    Resize(u16, u16),
    /// Periodic tick for animations / background updates.
    Tick,
}

/// Polls for terminal events with a configurable tick rate.
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }

    /// Block until the next event (key press, resize, or tick timeout).
    pub fn next(&self) -> Result<AppEvent> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                CrosstermEvent::Key(key) => Ok(AppEvent::Key(key)),
                CrosstermEvent::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}
