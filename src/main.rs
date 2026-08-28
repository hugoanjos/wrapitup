mod app;
mod auth;
mod cli;
mod context;
mod sink;
mod ui;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use librespot::connect::{
    ConnectConfig, LoadContextOptions, LoadRequest, LoadRequestOptions, Options, Spirc,
};
use librespot::core::config::DeviceType;
use librespot::core::{Session, SessionConfig};
use librespot::playback::config::PlayerConfig;
use librespot::playback::mixer::softmixer::SoftMixer;
use librespot::playback::mixer::{Mixer, MixerConfig};
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
    let context_uri = uri.to_uri().map_err(|e| anyhow!("bad Spotify URI: {e}"))?;

    println!("Authenticating with Spotify...");
    let (creds, cache) = auth::credentials(&config_dir, cli.oauth_port).await?;

    // autoplay off: when the album/playlist ends, don't roll into Spotify's
    // "autoplay similar songs" and keep racking up plays the user didn't choose.
    let session_config = SessionConfig {
        autoplay: Some(false),
        ..Default::default()
    };
    let session = Session::new(session_config, Some(cache));

    let mixer: Arc<dyn Mixer> = Arc::new(
        SoftMixer::open(MixerConfig::default()).map_err(|e| anyhow!("init mixer: {e}"))?,
    );

    let player_config = PlayerConfig {
        position_update_interval: Some(Duration::from_millis(500)),
        ..Default::default()
    };
    let player = Player::new(
        player_config,
        session.clone(),
        mixer.get_soft_volume(),
        sink::null_sink,
    );
    let events = player.get_player_event_channel();

    let connect_config = ConnectConfig {
        name: "wrapitup".to_string(),
        device_type: DeviceType::Computer,
        ..Default::default()
    };

    println!("Registering as a Spotify Connect device...");
    let (spirc, spirc_task) = Spirc::new(connect_config, session.clone(), creds, player, mixer)
        .await
        .map_err(|e| anyhow!("starting Spotify Connect: {e}"))?;

    // The session is connected now; fetch a friendly label for the header.
    let label = context::label(&session, &uri).await.unwrap_or_else(|e| {
        log::warn!("could not label context: {e}");
        context_uri.clone()
    });

    let context_options = cli.shuffle.then(|| {
        LoadContextOptions::Options(Options {
            shuffle: true,
            ..Default::default()
        })
    });
    spirc
        .activate()
        .map_err(|e| anyhow!("activating Connect device: {e}"))?;
    spirc
        .load(LoadRequest::from_context_uri(
            context_uri,
            LoadRequestOptions {
                start_playing: true,
                context_options,
                ..Default::default()
            },
        ))
        .map_err(|e| anyhow!("loading context: {e}"))?;
    let _ = spirc.play();

    let outcome = app::run(label, cli.quit_on_finish, spirc, spirc_task, events).await;

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
