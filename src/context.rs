//! Resolve a link into a human label for the header. Playback itself is driven by
//! Spirc from the context URI, so this only needs the top-level object (one
//! request), not the full track list.

use anyhow::{Result, anyhow, bail};
use librespot::core::{Session, SpotifyUri};
use librespot::metadata::{Album, Metadata, Playlist, Track};

pub async fn label(session: &Session, uri: &SpotifyUri) -> Result<String> {
    match uri {
        SpotifyUri::Track { .. } => {
            let track = Track::get(session, uri)
                .await
                .map_err(|e| anyhow!("fetching track: {e}"))?;
            let artist = track
                .artists
                .first()
                .map(|a| a.name.as_str())
                .unwrap_or("Unknown artist");
            Ok(format!("{artist} — {}", track.name))
        }
        SpotifyUri::Album { .. } => {
            let album = Album::get(session, uri)
                .await
                .map_err(|e| anyhow!("fetching album: {e}"))?;
            let artist = album
                .artists
                .first()
                .map(|a| a.name.as_str())
                .unwrap_or("Unknown artist");
            Ok(format!("{artist} — {}", album.name))
        }
        SpotifyUri::Playlist { .. } => {
            let playlist = Playlist::get(session, uri)
                .await
                .map_err(|e| anyhow!("fetching playlist: {e}"))?;
            Ok(playlist.name().to_owned())
        }
        other => bail!("{} links aren't supported yet", other.item_type()),
    }
}
