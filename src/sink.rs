//! A no-op audio backend.
//!
//! librespot still fetches and decodes the real audio stream (which is what makes
//! Spotify count it as a genuine play); we just drop the decoded samples instead
//! of sending them to a speaker.

use librespot::playback::audio_backend::{Sink, SinkResult};
use librespot::playback::convert::Converter;
use librespot::playback::decoder::AudioPacket;

pub struct NullSink;

impl Sink for NullSink {
    fn write(&mut self, _packet: AudioPacket, _converter: &mut Converter) -> SinkResult<()> {
        Ok(())
    }
}

/// Builder handed to `Player::new`. A typed `fn` so it coerces to the closure
/// bound `FnOnce() -> Box<dyn Sink>` without an explicit cast at the call site.
pub fn null_sink() -> Box<dyn Sink> {
    Box::new(NullSink)
}
