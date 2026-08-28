mod app;
mod auth;
mod cli;
mod resolve;
mod sink;
mod ui;

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use librespot::playback::config::PlayerConfig;
use librespot::playback::mixer::NoOpVolume;
use librespot::playback::player::Player;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let config_dir = auth::config_dir()?;
    init_logging(&config_dir);

    if let Some(cli::Cmd::Logout) = cli.cmd {
        let file = auth::credentials_file(&config_dir);
        match std::fs::remove_file(&file) {
            Ok(()) => println!("Removed cached credentials ({}).", file.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("No cached credentials to remove.");
            }
            Err(e) => return Err(e).context("removing credentials file"),
        }
        return Ok(());
    }

    let target = cli
        .target
        .as_deref()
        .context("no Spotify link given (try `wrapitup --help`)")?;
    let uri = cli::parse_target(target)?;

    println!("Authenticating with Spotify...");
    let session = auth::connect(&config_dir, cli.oauth_port).await?;

    let pretty = uri.to_uri().unwrap_or_else(|_| target.to_string());
    println!("Resolving {pretty}...");
    let mut resolved = resolve::resolve(&session, &uri).await?;

    if cli.shuffle {
        use rand::seq::SliceRandom;
        resolved.tracks.shuffle(&mut rand::thread_rng());
    }

    println!(
        "Playing {} track(s) silently. Starting player...",
        resolved.tracks.len()
    );

    let player_config = PlayerConfig {
        position_update_interval: Some(Duration::from_millis(500)),
        ..Default::default()
    };
    let player = Player::new(
        player_config,
        session.clone(),
        Box::new(NoOpVolume),
        sink::null_sink,
    );
    let events = player.get_player_event_channel();

    let outcome = app::run(
        resolved.tracks,
        resolved.context_name,
        cli.quit_on_finish,
        player.clone(),
        events,
    )
    .await;

    session.shutdown();
    outcome
}

fn init_logging(config_dir: &Path) {
    let path = config_dir.join("wrapitup.log");
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,librespot_core::channel=warn"),
    )
    .target(env_logger::Target::Pipe(Box::new(file)))
    .format_timestamp_secs()
    .try_init();
}
