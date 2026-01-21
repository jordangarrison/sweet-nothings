//! TUI module
//!
//! Provides the terminal user interface using ratatui.

mod app;
mod ui;

pub use app::App;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

use crate::audio::CpalRecorder;
use crate::clipboard::SystemClipboard;
use crate::config::Config;

/// Run the TUI application
pub fn run(config: &Config) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let recorder = CpalRecorder::with_temp_dir();
    let clipboard = SystemClipboard::new()?;

    let mut app = App::new(recorder, clipboard, config.clone());

    // Run the main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Auto-paste if enabled (after terminal is restored)
    if app.should_paste_on_exit() {
        // Small delay to let terminal fully restore focus
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = app.do_paste() {
            eprintln!("Warning: Failed to auto-paste: {}", e);
        }
    }

    // Return result or propagate error
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        return Err(e);
    }

    // Print the transcription result
    if let Some(text) = app.transcription_result() {
        if !text.is_empty() {
            println!("Transcription: {}", text);
        }
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    // Start recording immediately
    app.start_recording()?;

    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Check for background task completion
        app.check_transcription_result();

        // Handle events with a timeout for responsive UI
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            if app.is_recording() {
                                app.stop_recording()?;
                            } else if app.is_done() || app.is_error() {
                                break;
                            }
                        }
                        KeyCode::Char('q') => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Auto-exit after result is shown
        if app.should_exit() {
            break;
        }
    }

    Ok(())
}
