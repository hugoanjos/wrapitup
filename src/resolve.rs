use anyhow::{Result, anyhow, bail};
use futures_util::stream::{self, StreamExt};
use librespot::core::{Session, SpotifyUri};
use librespot::metadata::{Album, Metadata, Playlist, Track};

/// Everything the UI needs to know about one track.
#[derive(Clone, Debug)]
pub struct TrackMeta {
    pub uri: SpotifyUri,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u32,
}

/// What we resolved a link into: a title for the header plus the ordered tracks.
pub struct Resolved {
    pub context_name: String,
    pub tracks: Vec<TrackMeta>,
}

const METADATA_CONCURRENCY: usize = 8;

pub async fn resolve(session: &Session, uri: &SpotifyUri) -> Result<Resolved> {
    match uri {
        SpotifyUri::Track { .. } => {
            let meta = fetch_track(session, uri.clone()).await?;
            Ok(Resolved {
                context_name: format!("{} — {}", meta.artist, meta.album),
                tracks: vec![meta],
            })
        }
        SpotifyUri::Album { .. } => {
            let album = Album::get(session, uri)
                .await
                .map_err(|e| anyhow!("fetching album: {e}"))?;
            let uris: Vec<SpotifyUri> = album.tracks().cloned().collect();
            let tracks = fetch_many(session, uris).await;
            ensure_nonempty(&tracks)?;
            let artist = album
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default();
            Ok(Resolved {
                context_name: format!("{artist} — {}", album.name),
                tracks,
            })
        }
        SpotifyUri::Playlist { .. } => {
            let playlist = Playlist::get(session, uri)
                .await
                .map_err(|e| anyhow!("fetching playlist: {e}"))?;
            let uris: Vec<SpotifyUri> = playlist.tracks().cloned().collect();
            let name = playlist.name().to_owned();
            let tracks = fetch_many(session, uris).await;
            ensure_nonempty(&tracks)?;
            Ok(Resolved {
                context_name: name,
                tracks,
            })
        }
        other => bail!("{} links aren't supported yet", other.item_type()),
    }
}

fn ensure_nonempty(tracks: &[TrackMeta]) -> Result<()> {
    if tracks.is_empty() {
        bail!("no playable tracks found for that link");
    }
    Ok(())
}

/// Fetch track metadata for many URIs concurrently, preserving order. Tracks that
/// fail to resolve are dropped (with a log line) rather than aborting the whole run.
async fn fetch_many(session: &Session, uris: Vec<SpotifyUri>) -> Vec<TrackMeta> {
    stream::iter(uris)
        .map(|uri| {
            let session = session.clone();
            async move {
                match fetch_track(&session, uri.clone()).await {
                    Ok(meta) => Some(meta),
                    Err(e) => {
                        log::warn!("skipping track {uri}: {e}");
                        None
                    }
                }
            }
        })
        .buffered(METADATA_CONCURRENCY)
        .filter_map(|opt| async move { opt })
        .collect()
        .await
}

async fn fetch_track(session: &Session, uri: SpotifyUri) -> Result<TrackMeta> {
    let track = Track::get(session, &uri)
        .await
        .map_err(|e| anyhow!("fetching track metadata: {e}"))?;

    let artist = track
        .artists
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "Unknown artist".to_owned());

    Ok(TrackMeta {
        uri,
        title: track.name,
        artist,
        album: track.album.name,
        duration_ms: track.duration.max(0) as u32,
    })
}
