use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig};
use librespot::oauth::OAuthClientBuilder;

/// Scopes requested during the browser login. `streaming` is the one that matters
/// for opening a playback session against Spotify's access point.
const OAUTH_SCOPES: &[&str] = &[
    "streaming",
    "user-read-email",
    "user-read-private",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
];

pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("could not locate a config directory for this OS")?
        .join("wrapitup");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating config dir {}", dir.display()))?;
    Ok(dir)
}

pub fn credentials_file(config_dir: &Path) -> PathBuf {
    config_dir.join("credentials.json")
}

/// Obtain Spotify credentials plus the cache they live in. The caller passes both
/// to `Spirc::new`, which performs the actual session connect and re-saves a
/// long-lived credential blob into the cache.
pub async fn credentials(config_dir: &Path, oauth_port: u16) -> Result<(Credentials, Cache)> {
    let cache = Cache::new(Some(config_dir), None::<&Path>, None::<&Path>, None)
        .map_err(|e| anyhow!("opening credential cache: {e}"))?;

    if let Some(cached) = cache.credentials() {
        if probe(cached.clone()).await {
            log::info!("using cached Spotify credentials");
            return Ok((cached, cache));
        }
        log::warn!("cached credentials rejected; starting browser login");
    }

    let token = browser_login(oauth_port).await?;
    Ok((Credentials::with_access_token(token), cache))
}

/// Cheaply check whether cached credentials still authenticate, on a throwaway
/// session, so we can fall back to the browser flow before handing them to Spirc.
async fn probe(creds: Credentials) -> bool {
    let session = Session::new(SessionConfig::default(), None);
    match session.connect(creds, false).await {
        Ok(()) => {
            session.shutdown();
            true
        }
        Err(_) => false,
    }
}

async fn browser_login(oauth_port: u16) -> Result<String> {
    let client_id = SessionConfig::default().client_id;
    let redirect_uri = format!("http://127.0.0.1:{oauth_port}/login");

    let client = OAuthClientBuilder::new(&client_id, &redirect_uri, OAUTH_SCOPES.to_vec())
        .open_in_browser()
        .build()
        .map_err(|e| anyhow!("building OAuth client: {e}"))?;

    println!("Opening your browser to log in to Spotify (one time only)...");

    let token = tokio::task::spawn_blocking(move || client.get_access_token())
        .await
        .context("OAuth task panicked")?
        .map_err(|e| anyhow!("browser login failed: {e}"))?;

    Ok(token.access_token)
}
