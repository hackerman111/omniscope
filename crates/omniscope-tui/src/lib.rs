pub mod app;
pub mod event;
pub mod popup;
pub mod ui;
pub mod keys;
pub mod command;
pub mod panels;
pub mod theme;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;
use event::{AppEvent, EventHandler};

/// Run the full TUI application.
pub fn run_tui(app: &mut App) -> Result<()> {
    // Install panic hook
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen);
        original_hook(info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let event_handler = EventHandler::new(Duration::from_millis(250));

    // Main loop
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        match event_handler.next()? {
            AppEvent::Key(key) => keys::handle_key(app, key.code, key.modifiers),
            AppEvent::Resize(_, _) => {}
            AppEvent::Tick => {}
        }

        // Handle pending editor launch (requires terminal suspension)
        if let Some(path) = app.pending_editor_path.take() {
            // Suspend TUI
            disable_raw_mode()?;
            io::stdout().execute(LeaveAlternateScreen)?;

            // Launch editor
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            let status = std::process::Command::new(&editor)
                .arg(&path)
                .status();

            match status {
                Ok(s) if s.success() => {
                    app.status_message = format!("Opened in {editor}");
                    // Reload the card in case user edited it
                    app.refresh_books();
                }
                Ok(s) => {
                    app.status_message = format!("{editor} exited with: {s}");
                }
                Err(e) => {
                    app.status_message = format!("Failed to run {editor}: {e}");
                }
            }

            // Restore TUI
            enable_raw_mode()?;
            io::stdout().execute(EnterAlternateScreen)?;
            terminal.clear()?;
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
