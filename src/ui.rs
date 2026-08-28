use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Gauge, Paragraph};

use crate::app::{App, State};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let block = Block::bordered()
        .title(format!(" wrapitup — {} ", app.context_label))
        .title_bottom(format!(" {} elapsed ", fmt_clock(app.started.elapsed())));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1), // spacer
        Constraint::Length(1), // title
        Constraint::Length(1), // artist
        Constraint::Length(1), // album · track n
        Constraint::Length(1), // spacer
        Constraint::Length(1), // status + progress
        Constraint::Min(0),    // spacer
        Constraint::Length(1), // help
    ])
    .horizontal_margin(2)
    .split(inner);

    let now = &app.now;

    if app.state == State::Connecting {
        f.render_widget(Paragraph::new("connecting to Spotify…".dim()), rows[1]);
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                "♪  ".dim(),
                display_or(&now.title, "—").bold(),
            ])),
            rows[1],
        );
        f.render_widget(Paragraph::new(now.artist.clone().dim()), rows[2]);
        let mut third = now.album.clone();
        if let Some(n) = now.track_no {
            if !third.is_empty() {
                third.push_str("  ·  ");
            }
            third.push_str(&format!("track {n}"));
        }
        f.render_widget(Paragraph::new(third.dim()), rows[3]);
    }

    let status = match app.state {
        State::Connecting | State::Loading => "…",
        State::Playing => "▶",
        State::Paused => "⏸",
        State::Finished => "■",
    };
    let pos = app.display_pos_ms();
    let dur = now.duration_ms.max(1);
    let ratio = (f64::from(pos) / f64::from(dur)).clamp(0.0, 1.0);

    let cols = Layout::horizontal([
        Constraint::Length(1), // status glyph
        Constraint::Length(5), // elapsed
        Constraint::Min(10),   // gauge
        Constraint::Length(5), // total
    ])
    .spacing(1)
    .split(rows[5]);

    f.render_widget(Paragraph::new(status), cols[0]);
    f.render_widget(Paragraph::new(fmt_ms(pos)).right_aligned(), cols[1]);
    f.render_widget(Gauge::default().ratio(ratio).label(""), cols[2]);
    f.render_widget(Paragraph::new(fmt_ms(now.duration_ms)), cols[3]);

    let help = if app.state == State::Finished {
        "context finished  ·  q quit"
    } else {
        "p play/pause  ·  n next  ·  b prev  ·  q quit"
    };
    f.render_widget(Paragraph::new(help.dim()), rows[7]);
}

fn display_or(s: &str, fallback: &str) -> String {
    if s.is_empty() {
        fallback.to_owned()
    } else {
        s.to_owned()
    }
}

fn fmt_ms(ms: u32) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn fmt_clock(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
