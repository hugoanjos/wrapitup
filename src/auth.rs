use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use librespot::core::{Session, SessionConfig};
use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
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

/// Connect to Spotify, reusing cached credentials when possible and falling back
/// to a one-time browser OAuth login.
pub async fn connect(config_dir: &Path, oauth_port: u16) -> Result<Session> {
    let cache = Cache::new(Some(config_dir), None::<&Path>, None::<&Path>, None)
        .map_err(|e| anyhow!("opening credential cache: {e}"))?;

    if let Some(creds) = cache.credentials() {
        let session = Session::new(SessionConfig::default(), Some(cache.clone()));
        match session.connect(creds, false).await {
            Ok(()) => {
                log::info!("authenticated with cached credentials");
                return Ok(session);
            }
            Err(e) => {
                log::warn!("cached credentials rejected ({e}); starting browser login");
            }
        }
    }

    let token = browser_login(oauth_port).await?;

    let session = Session::new(SessionConfig::default(), Some(cache));
    session
        .connect(Credentials::with_access_token(token), true)
        .await
        .map_err(|e| anyhow!("connecting to Spotify with new credentials: {e}"))?;
    log::info!("authenticated via browser login; credentials cached");
    Ok(session)
}

async fn browser_login(oauth_port: u16) -> Result<String> {
    let client_id = SessionConfig::default().client_id;
    let redirect_uri = format!("http://127.0.0.1:{oauth_port}/login");

    let client = OAuthClientBuilder::new(&client_id, &redirect_uri, OAUTH_SCOPES.to_vec())
        .open_in_browser()
        .build()
        .map_err(|e| anyhow!("building OAuth client: {e}"))?;

    println!("Opening your browser to log in to Spotify (one time only)...");

    // `get_access_token` blocks on a local TCP listener, so keep it off the async
    // executor.
    let token = tokio::task::spawn_blocking(move || client.get_access_token())
        .await
        .context("OAuth task panicked")?
        .map_err(|e| anyhow!("browser login failed: {e}"))?;

    Ok(token.access_token)
}
