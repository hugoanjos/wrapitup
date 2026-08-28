# wrapitup

A terminal app that **plays a Spotify album, track, or playlist with no audio**, so
your listening still counts toward Spotify Wrapped / your monthly capsule while you
actually listen somewhere else (Apple Music, vinyl, etc.).

It embeds [librespot](https://github.com/librespot-org/librespot) as a real Spotify
Connect device — it authenticates with your Premium account and streams the genuine
audio from Spotify's servers (which is what makes a play count), then throws the
decoded samples away instead of sending them to your speakers. Tracks play their
full length so timestamps stay realistic.

```
wrapitup https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy
```

## Requirements

- Spotify **Premium** (librespot won't stream on free accounts)
- Rust toolchain (via [rustup](https://rustup.rs))

## Install

```sh
cargo install --path .
# or just: cargo build --release  ->  ./target/release/wrapitup
```

## Usage

```sh
wrapitup <spotify link or uri>      # album, track, or playlist
wrapitup --shuffle <link>           # shuffle track order
wrapitup --quit-on-finish <link>    # exit when the last track ends
wrapitup logout                     # forget the cached login
```

Accepted link forms: `spotify:album:…`, `https://open.spotify.com/album/…`,
`https://open.spotify.com/intl-xx/track/…`, scheme-less `open.spotify.com/…`, and
legacy `/user/…/playlist/…`.

### Controls

| key             | action        |
|-----------------|---------------|
| `p` / `space`   | play / pause  |
| `n` / `→`       | next track    |
| `b` / `←`       | previous track (or restart current if >3s in) |
| `q` / `esc`     | quit          |

## First run

The first launch opens your browser for a one-time Spotify login. Credentials are
cached under your OS config dir (`~/Library/Application Support/wrapitup` on macOS,
`~/.config/wrapitup` on Linux); later runs are silent. `wrapitup logout` clears them.

The OAuth redirect uses `http://127.0.0.1:5588/login` — change the port with
`--oauth-port` if 5588 is taken.

Logs go to `wrapitup.log` in that same config dir. Set `RUST_LOG=debug` for more.

## Caveats

- **Whether librespot plays count toward Wrapped is not guaranteed by Spotify.**
  In practice librespot/spotifyd streams do show up in listening history and
  Wrapped, but Spotify doesn't officially support this and could change it.
- Automating playback to generate stats sits in a gray area of Spotify's rules on
  artificial streaming. Mirroring music you genuinely listened to, 1:1 and at a
  realistic pace, is low risk, but it's your call.
