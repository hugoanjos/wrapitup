use std::future::Future;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use librespot::connect::Spirc;
use librespot::metadata::audio::UniqueFields;
use librespot::playback::player::{PlayerEvent, PlayerEventChannel};

use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Connecting,
    Loading,
    Playing,
    Paused,
    Finished,
}

/// What the UI shows about the current track. Populated entirely from
/// `PlayerEvent`s (Spirc owns the playlist).
#[derive(Default)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_no: Option<u32>,
    pub duration_ms: u32,
}

pub struct App {
    pub context_label: String,
    pub now: NowPlaying,
    pub state: State,
    pub started: Instant,
    pos_ms: u32,
    anchor: Instant,
    pending_end: bool,
    quit_on_finish: bool,
    should_quit: bool,
}

impl App {
    fn new(context_label: String, quit_on_finish: bool) -> Self {
        App {
            context_label,
            now: NowPlaying::default(),
            state: State::Connecting,
            started: Instant::now(),
            pos_ms: 0,
            anchor: Instant::now(),
            pending_end: false,
            quit_on_finish,
            should_quit: false,
        }
    }

    pub fn display_pos_ms(&self) -> u32 {
        let dur = self.now.duration_ms;
        match self.state {
            State::Playing => {
                let extra = self.anchor.elapsed().as_millis() as u64;
                (u64::from(self.pos_ms) + extra).min(u64::from(dur)) as u32
            }
            _ => self.pos_ms.min(dur.max(self.pos_ms)),
        }
    }

    fn set_pos(&mut self, ms: u32) {
        self.pos_ms = ms;
        self.anchor = Instant::now();
    }

    fn on_key(&mut self, spirc: &Spirc, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            KeyCode::Char('p') | KeyCode::Char(' ') => {
                let _ = spirc.play_pause();
            }
            KeyCode::Char('n') | KeyCode::Right => {
                let _ = spirc.next();
            }
            KeyCode::Char('b') | KeyCode::Left => {
                let _ = spirc.prev();
            }
            _ => {}
        }
    }

    fn on_player_event(&mut self, ev: PlayerEvent) {
        match ev {
            PlayerEvent::TrackChanged { audio_item } => {
                self.pending_end = false;
                self.now.title = audio_item.name.clone();
                self.now.duration_ms = audio_item.duration_ms;
                match &audio_item.unique_fields {
                    UniqueFields::Track {
                        artists,
                        album,
                        number,
                        ..
                    } => {
                        self.now.artist = artists
                            .first()
                            .map(|a| a.name.clone())
                            .unwrap_or_default();
                        self.now.album = album.clone();
                        self.now.track_no = Some(*number);
                    }
                    UniqueFields::Episode { show_name, .. } => {
                        self.now.artist = show_name.clone();
                        self.now.album.clear();
                        self.now.track_no = None;
                    }
                    UniqueFields::Local { artists, album, .. } => {
                        self.now.artist = artists.clone().unwrap_or_default();
                        self.now.album = album.clone().unwrap_or_default();
                        self.now.track_no = None;
                    }
                }
            }
            PlayerEvent::Loading { position_ms, .. } => {
                self.pending_end = false;
                if self.state != State::Playing {
                    self.state = State::Loading;
                }
                self.set_pos(position_ms);
            }
            PlayerEvent::Playing { position_ms, .. } => {
                self.pending_end = false;
                self.state = State::Playing;
                self.set_pos(position_ms);
            }
            PlayerEvent::Paused { position_ms, .. } => {
                self.state = State::Paused;
                self.set_pos(position_ms);
            }
            PlayerEvent::PositionChanged { position_ms, .. }
            | PlayerEvent::PositionCorrection { position_ms, .. }
            | PlayerEvent::Seeked { position_ms, .. } => {
                self.set_pos(position_ms);
            }
            PlayerEvent::EndOfTrack { .. } => {
                self.pending_end = true;
            }
            PlayerEvent::Stopped { .. } => {
                if self.pending_end || self.state == State::Playing {
                    self.state = State::Finished;
                    if self.quit_on_finish {
                        self.should_quit = true;
                    }
                }
            }
            PlayerEvent::SessionConnected { .. } if self.state == State::Connecting => {
                self.state = State::Loading;
            }
            _ => {}
        }
    }

    /// Fallback finish detection: EndOfTrack with no following track for a while.
    fn poll_pending_end(&mut self) {
        if self.pending_end
            && self.state != State::Playing
            && self.state != State::Finished
            && self.anchor.elapsed() > Duration::from_secs(2)
        {
            self.state = State::Finished;
            if self.quit_on_finish {
                self.should_quit = true;
            }
        }
    }
}

/// Own the terminal, run the event loop, and always restore the terminal on exit.
pub async fn run(
    context_label: String,
    quit_on_finish: bool,
    spirc: Spirc,
    spirc_task: impl Future<Output = ()>,
    mut events: PlayerEventChannel,
) -> Result<()> {
    let mut app = App::new(context_label, quit_on_finish);

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app, &spirc, spirc_task, &mut events).await;
    ratatui::restore();

    let _ = spirc.shutdown();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    spirc: &Spirc,
    spirc_task: impl Future<Output = ()>,
    events: &mut PlayerEventChannel,
) -> Result<()> {
    let mut input = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    tokio::pin!(spirc_task);

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            // Ask Spirc to disconnect cleanly (drops us from the user's device
            // list) and give it a moment to send that state before we bail.
            let _ = spirc.shutdown();
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut spirc_task).await;
            return Ok(());
        }

        tokio::select! {
            _ = &mut spirc_task => return Ok(()),
            maybe_input = input.next() => match maybe_input {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    app.on_key(spirc, key.code, key.modifiers);
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            },
            Some(pe) = events.recv() => app.on_player_event(pe),
            _ = ticker.tick() => app.poll_pending_end(),
            _ = tokio::signal::ctrl_c() => app.should_quit = true,
        }
    }
}
