use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use librespot::core::SpotifyUri;

#[derive(Parser, Debug)]
#[command(
    name = "wrapitup",
    version,
    about = "Silently stream a Spotify album/track/playlist so it still counts toward your stats",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// Spotify URL or URI: an album, a track, or a playlist
    ///
    /// e.g. https://open.spotify.com/album/xxxx  or  spotify:track:xxxx
    pub target: Option<String>,

    /// Shuffle the track order before playing
    #[arg(long)]
    pub shuffle: bool,

    /// Quit automatically once the last track finishes
    #[arg(long)]
    pub quit_on_finish: bool,

    /// Local port for the one-time OAuth browser redirect
    #[arg(long, default_value_t = 5588)]
    pub oauth_port: u16,

    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Forget the cached Spotify credentials (next run re-does the browser login)
    Logout,
}

/// Turn whatever the user pasted into a canonical `SpotifyUri`.
///
/// Accepts:
///   - `spotify:album:ID` / `spotify:track:ID` / `spotify:playlist:ID`
///   - `https://open.spotify.com/album/ID?si=...`
///   - `https://open.spotify.com/intl-pt/track/ID`
///   - `https://open.spotify.com/user/NAME/playlist/ID` (legacy)
///   - a bare `open.spotify.com/...` without the scheme
pub fn parse_target(input: &str) -> Result<SpotifyUri> {
    let s = input.trim();

    if s.starts_with("spotify:") {
        return SpotifyUri::from_uri(s)
            .map_err(|e| anyhow::anyhow!("not a valid Spotify URI ({s:?}): {e}"));
    }

    let without_scheme = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);

    let path = without_scheme
        .strip_prefix("open.spotify.com/")
        .or_else(|| without_scheme.strip_prefix("play.spotify.com/"))
        .with_context(|| format!("don't know how to read {s:?} as a Spotify link"))?;

    // Drop query string / fragment, then split into path segments.
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let mut segs = path.split('/').filter(|seg| !seg.is_empty());

    let mut first = segs.next().context("empty Spotify link path")?;
    if first.starts_with("intl-") {
        // locale prefix like /intl-pt/album/...
        first = segs.next().context("nothing after locale prefix in link")?;
    }

    let (kind, id) = if first == "user" {
        let _user = segs.next().context("truncated /user/ link")?;
        let kind = segs.next().context("truncated /user/ link")?;
        let id = segs.next().context("truncated /user/ link")?;
        (kind, id)
    } else {
        let id = segs.next().context("Spotify link has a type but no ID")?;
        (first, id)
    };

    match kind {
        "album" | "track" | "playlist" | "episode" | "show" => {}
        other => bail!("unsupported link type {other:?} (use an album, track, or playlist)"),
    }

    let uri = format!("spotify:{kind}:{id}");
    SpotifyUri::from_uri(&uri).map_err(|e| anyhow::anyhow!("could not parse {uri:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALBUM: &str = "4aawyAB9vmqN3uQ7FjRGTy";
    const TRACK: &str = "11dFghVXANMlKmJXsNCbNl";
    const PLAYLIST: &str = "37i9dQZF1DXcBWIGoYBM5M";

    fn kind_of(uri: &SpotifyUri) -> &'static str {
        uri.item_type()
    }

    #[test]
    fn parses_bare_uri() {
        let u = parse_target(&format!("spotify:track:{TRACK}")).unwrap();
        assert_eq!(kind_of(&u), "track");
    }

    #[test]
    fn parses_https_album_with_query() {
        let u = parse_target(&format!("https://open.spotify.com/album/{ALBUM}?si=abc123")).unwrap();
        assert_eq!(kind_of(&u), "album");
    }

    #[test]
    fn parses_locale_prefixed_link() {
        let u =
            parse_target(&format!("https://open.spotify.com/intl-pt/track/{TRACK}")).unwrap();
        assert_eq!(kind_of(&u), "track");
    }

    #[test]
    fn parses_scheme_less_link() {
        let u = parse_target(&format!("open.spotify.com/playlist/{PLAYLIST}")).unwrap();
        assert_eq!(kind_of(&u), "playlist");
    }

    #[test]
    fn parses_legacy_user_playlist_link() {
        let u = parse_target(&format!(
            "https://open.spotify.com/user/spotify/playlist/{PLAYLIST}"
        ))
        .unwrap();
        assert_eq!(kind_of(&u), "playlist");
    }

    #[test]
    fn rejects_non_spotify_input() {
        assert!(parse_target("https://example.com/album/x").is_err());
        assert!(parse_target("hello world").is_err());
    }

    #[test]
    fn rejects_unsupported_type() {
        assert!(parse_target("https://open.spotify.com/artist/x").is_err());
    }
}
