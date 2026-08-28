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
        .title(format!(" wrapitup — {} ", app.context_name))
        .title_bottom(format!(" {} elapsed ", fmt_clock(app.started.elapsed())));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1), // spacer
        Constraint::Length(1), // title
        Constraint::Length(1), // artist
        Constraint::Length(1), // album · track n/N
        Constraint::Length(1), // spacer
        Constraint::Length(1), // status + progress
        Constraint::Min(0),    // spacer
        Constraint::Length(1), // help
    ])
    .horizontal_margin(2)
    .split(inner);

    let track = app.cur();

    f.render_widget(
        Paragraph::new(Line::from(vec!["♪  ".dim(), track.title.clone().bold()])),
        rows[1],
    );
    f.render_widget(Paragraph::new(track.artist.clone().dim()), rows[2]);
    f.render_widget(
        Paragraph::new(
            format!(
                "{}  ·  track {}/{}",
                track.album,
                app.idx + 1,
                app.tracks.len()
            )
            .dim(),
        ),
        rows[3],
    );

    let status = match app.state {
        State::Loading => "…",
        State::Playing => "▶",
        State::Paused => "⏸",
        State::Finished => "■",
    };
    let pos = app.display_pos_ms();
    let dur = track.duration_ms.max(1);
    let ratio = (f64::from(pos) / f64::from(dur)).clamp(0.0, 1.0);

    let cols = Layout::horizontal([
        Constraint::Length(1),  // status glyph
        Constraint::Length(5),  // elapsed
        Constraint::Min(10),    // gauge
        Constraint::Length(5),  // total
    ])
    .spacing(1)
    .split(rows[5]);

    f.render_widget(Paragraph::new(status), cols[0]);
    f.render_widget(Paragraph::new(fmt_ms(pos)).right_aligned(), cols[1]);
    f.render_widget(Gauge::default().ratio(ratio).label(""), cols[2]);
    f.render_widget(Paragraph::new(fmt_ms(dur)), cols[3]);

    let help = if app.state == State::Finished {
        "p replay  ·  b prev  ·  q quit"
    } else {
        "p play/pause  ·  n next  ·  b prev  ·  q quit"
    };
    f.render_widget(Paragraph::new(help.dim()), rows[7]);
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
