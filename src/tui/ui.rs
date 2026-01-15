//! UI rendering

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppState};

/// Draw the entire UI
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Create a centered popup
    let popup_area = centered_rect(60, 50, area);

    // Choose color based on state
    let border_color = match app.state() {
        AppState::Recording => Color::Red,
        AppState::Transcribing => Color::Yellow,
        AppState::Done => Color::Green,
        AppState::Error => Color::Magenta,
    };

    let title = match app.state() {
        AppState::Recording => " Recording ",
        AppState::Transcribing => " Transcribing ",
        AppState::Done => " Done ",
        AppState::Error => " Error ",
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Get inner area for content
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Draw content based on state
    match app.state() {
        AppState::Recording => draw_recording(f, app, inner),
        AppState::Transcribing => draw_transcribing(f, app, inner),
        AppState::Done => draw_done(f, app, inner),
        AppState::Error => draw_error(f, app, inner),
    }
}

fn draw_recording(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Timer
            Constraint::Length(3), // Audio level
            Constraint::Min(1),    // Spacer
            Constraint::Length(2), // Instructions
        ])
        .split(area);

    // Timer
    let duration = app.recording_duration();
    let timer_text = format!(
        "{:02}:{:02}",
        duration.as_secs() / 60,
        duration.as_secs() % 60
    );
    let timer = Paragraph::new(timer_text)
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(timer, chunks[0]);

    // Audio level gauge
    let level = (app.audio_level() * 100.0) as u16;
    let gauge = Gauge::default()
        .block(Block::default().title("Audio Level"))
        .gauge_style(
            Style::default()
                .fg(Color::Red)
                .bg(Color::DarkGray),
        )
        .percent(level.min(100));
    f.render_widget(gauge, chunks[1]);

    // Instructions
    let instructions = Paragraph::new("Press Enter or Esc to stop recording")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(instructions, chunks[3]);
}

fn draw_transcribing(f: &mut Frame, _app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(1),    // Spacer
            Constraint::Length(3), // Spinner
            Constraint::Min(1),    // Spacer
        ])
        .split(area);

    // Simple spinner text
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let spinner_text = format!("{} Transcribing...", spinner_frames[frame_idx]);
    let spinner = Paragraph::new(spinner_text)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(spinner, chunks[1]);
}

fn draw_done(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Status
            Constraint::Min(1),    // Transcription
            Constraint::Length(2), // Instructions
        ])
        .split(area);

    // Status
    let status = Paragraph::new("Copied to clipboard!")
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(status, chunks[0]);

    // Transcription
    if let Some(text) = app.transcription() {
        let transcription = Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        f.render_widget(transcription, chunks[1]);
    }

    // Instructions
    let instructions = Paragraph::new("Press Enter, Esc, or q to exit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(instructions, chunks[2]);
}

fn draw_error(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Title
            Constraint::Min(1),    // Error message
            Constraint::Length(2), // Instructions
        ])
        .split(area);

    // Title
    let title = Paragraph::new("An error occurred")
        .style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Error message
    if let Some(error) = app.error_message() {
        let error_text = Paragraph::new(error)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Red));
        f.render_widget(error_text, chunks[1]);
    }

    // Instructions
    let instructions = Paragraph::new("Press Enter, Esc, or q to exit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(instructions, chunks[2]);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
