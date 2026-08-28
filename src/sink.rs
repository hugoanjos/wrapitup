//! A silent audio backend that still runs at real time.
//!
//! librespot fetches and decodes the genuine audio stream (which is what makes
//! Spotify count it as a real play); we drop the decoded samples instead of
//! sending them to a speaker.
//!
//! A real backend's `write()` blocks until the sound card has played the samples,
//! and that back-pressure is what paces playback. With nothing to block on, the
//! player would decode as fast as the CPU allows (many times real time), so here
//! `write()` sleeps for each packet's real duration against a running deadline.

use std::thread;
use std::time::{Duration, Instant};

use librespot::playback::SAMPLES_PER_SECOND;
use librespot::playback::audio_backend::{Sink, SinkResult};
use librespot::playback::convert::Converter;
use librespot::playback::decoder::AudioPacket;

/// If we're behind the deadline by more than this, assume a real gap (a pause, a
/// track change, a stall) and restart the clock instead of trying to catch up.
const RESYNC_AFTER: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct NullSink {
    /// Instant by which the audio written so far should have finished playing.
    next_deadline: Option<Instant>,
}

impl Sink for NullSink {
    fn start(&mut self) -> SinkResult<()> {
        self.next_deadline = None;
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.next_deadline = None;
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        let frames = match packet.samples() {
            Ok(samples) => samples.len(),
            Err(_) => return Ok(()), // raw/passthrough packets: not used here
        };
        if frames == 0 {
            return Ok(());
        }

        // Real-time length of this many interleaved f64 samples at 44.1 kHz stereo.
        let dur = Duration::from_secs_f64(frames as f64 / f64::from(SAMPLES_PER_SECOND));
        let now = Instant::now();

        match self.next_deadline {
            None => {
                self.next_deadline = Some(now + dur);
            }
            Some(deadline) if now < deadline => {
                thread::sleep(deadline - now);
                self.next_deadline = Some(deadline + dur);
            }
            Some(deadline) if now - deadline < RESYNC_AFTER => {
                // A little behind (decode overhead): don't sleep, but keep the
                // clock continuous so playback can't creep ahead of real time.
                self.next_deadline = Some(deadline + dur);
            }
            Some(_) => {
                // Real gap: restart the clock from now.
                self.next_deadline = Some(now + dur);
            }
        }

        Ok(())
    }
}

/// Builder handed to `Player::new`. A typed `fn` so it coerces to the closure
/// bound `FnOnce() -> Box<dyn Sink>` without an explicit cast at the call site.
pub fn null_sink() -> Box<dyn Sink> {
    Box::new(NullSink::default())
}
