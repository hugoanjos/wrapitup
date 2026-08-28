use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use librespot::playback::player::{Player, PlayerEvent, PlayerEventChannel};

use crate::resolve::TrackMeta;
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Loading,
    Playing,
    Paused,
    Finished,
}

pub struct App {
    pub tracks: Vec<TrackMeta>,
    pub context_name: String,
    pub idx: usize,
    pub state: State,
    pub started: Instant,
    unavailable: Vec<bool>,
    pos_ms: u32,
    anchor: Instant,
    play_req: Option<u64>,
    quit_on_finish: bool,
    should_quit: bool,
}

impl App {
    fn new(tracks: Vec<TrackMeta>, context_name: String, quit_on_finish: bool) -> Self {
        let n = tracks.len();
        App {
            tracks,
            context_name,
            idx: 0,
            state: State::Loading,
            started: Instant::now(),
            unavailable: vec![false; n],
            pos_ms: 0,
            anchor: Instant::now(),
            play_req: None,
            quit_on_finish,
            should_quit: false,
        }
    }

    pub fn cur(&self) -> &TrackMeta {
        &self.tracks[self.idx]
    }

    pub fn display_pos_ms(&self) -> u32 {
        let dur = self.cur().duration_ms;
        match self.state {
            State::Playing => {
                let extra = self.anchor.elapsed().as_millis() as u64;
                (self.pos_ms as u64 + extra).min(dur as u64) as u32
            }
            _ => self.pos_ms.min(dur),
        }
    }

    fn set_pos(&mut self, ms: u32) {
        self.pos_ms = ms.min(self.cur().duration_ms);
        self.anchor = Instant::now();
    }

    fn load(&mut self, player: &Player, idx: usize) {
        self.idx = idx;
        self.state = State::Loading;
        self.pos_ms = 0;
        self.anchor = Instant::now();
        self.play_req = None;
        player.load(self.tracks[idx].uri.clone(), true, 0);
    }

    fn next_available_from(&self, start: usize) -> Option<usize> {
        (start..self.tracks.len()).find(|&i| !self.unavailable[i])
    }

    fn prev_available_before(&self, end: usize) -> Option<usize> {
        (0..end).rev().find(|&i| !self.unavailable[i])
    }

    fn toggle(&mut self, player: &Player) {
        match self.state {
            State::Playing => {
                player.pause();
                self.state = State::Paused;
            }
            State::Paused => {
                player.play();
                self.state = State::Playing;
                self.anchor = Instant::now();
            }
            State::Finished => self.load(player, self.idx),
            State::Loading => {}
        }
    }

    fn next(&mut self, player: &Player) {
        if let Some(n) = self.next_available_from(self.idx + 1) {
            self.load(player, n);
        }
    }

    fn prev(&mut self, player: &Player) {
        if self.display_pos_ms() > 3000 {
            self.load(player, self.idx);
        } else if let Some(p) = self.prev_available_before(self.idx) {
            self.load(player, p);
        } else {
            self.load(player, self.idx);
        }
    }

    fn advance(&mut self, player: &Player) {
        if let Some(n) = self.next_available_from(self.idx + 1) {
            self.load(player, n);
        } else {
            self.state = State::Finished;
            let end = self.cur().duration_ms;
            self.set_pos(end);
            if self.quit_on_finish {
                self.should_quit = true;
            }
        }
    }

    fn is_current(&self, id: u64) -> bool {
        self.play_req == Some(id)
    }

    fn adopt(&mut self, id: u64) {
        if self.play_req.is_none() {
            self.play_req = Some(id);
        }
    }

    fn on_key(&mut self, player: &Player, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            KeyCode::Char('p') | KeyCode::Char(' ') => self.toggle(player),
            KeyCode::Char('n') | KeyCode::Right => self.next(player),
            KeyCode::Char('b') | KeyCode::Left => self.prev(player),
            _ => {}
        }
    }

    fn on_player_event(&mut self, player: &Player, ev: PlayerEvent) {
        match ev {
            PlayerEvent::Loading {
                play_request_id,
                position_ms,
                ..
            } => {
                self.adopt(play_request_id);
                if self.is_current(play_request_id) {
                    self.state = State::Loading;
                    self.set_pos(position_ms);
                }
            }
            PlayerEvent::Playing {
                play_request_id,
                position_ms,
                ..
            } => {
                self.adopt(play_request_id);
                if self.is_current(play_request_id) {
                    self.state = State::Playing;
                    self.set_pos(position_ms);
                }
            }
            PlayerEvent::Paused {
                play_request_id,
                position_ms,
                ..
            } => {
                if self.is_current(play_request_id) {
                    self.state = State::Paused;
                    self.set_pos(position_ms);
                }
            }
            PlayerEvent::PositionChanged {
                play_request_id,
                position_ms,
                ..
            }
            | PlayerEvent::PositionCorrection {
                play_request_id,
                position_ms,
                ..
            }
            | PlayerEvent::Seeked {
                play_request_id,
                position_ms,
                ..
            } => {
                if self.is_current(play_request_id) {
                    self.set_pos(position_ms);
                }
            }
            PlayerEvent::EndOfTrack {
                play_request_id, ..
            } => {
                if self.is_current(play_request_id) {
                    self.advance(player);
                }
            }
            PlayerEvent::Unavailable {
                play_request_id, ..
            } => {
                if self.is_current(play_request_id) {
                    self.unavailable[self.idx] = true;
                    self.advance(player);
                }
            }
            PlayerEvent::TimeToPreloadNextTrack {
                play_request_id, ..
            } => {
                if self.is_current(play_request_id)
                    && let Some(n) = self.next_available_from(self.idx + 1)
                {
                    player.preload(self.tracks[n].uri.clone());
                }
            }
            _ => {}
        }
    }
}

/// Own the terminal, run the event loop, and always restore the terminal on the way out.
pub async fn run(
    tracks: Vec<TrackMeta>,
    context_name: String,
    quit_on_finish: bool,
    player: std::sync::Arc<Player>,
    mut events: PlayerEventChannel,
) -> Result<()> {
    let mut app = App::new(tracks, context_name, quit_on_finish);
    let player_ref: &Player = &player;

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, player_ref, &mut events).await;
    ratatui::restore();

    // Stop streaming as soon as the UI is gone.
    player.stop();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    player: &Player,
    events: &mut PlayerEventChannel,
) -> Result<()> {
    app.load(player, 0);

    let mut input = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));

    loop {
        terminal.draw(|f| ui::render(f, app))?;
        if app.should_quit {
            return Ok(());
        }

        tokio::select! {
            maybe_input = input.next() => match maybe_input {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    app.on_key(player, key.code, key.modifiers);
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            },
            Some(pe) = events.recv() => app.on_player_event(player, pe),
            _ = ticker.tick() => {}
            _ = tokio::signal::ctrl_c() => app.should_quit = true,
        }
    }
}
