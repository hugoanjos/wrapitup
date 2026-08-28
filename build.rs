// This build script has no logic of its own. It exists so the manifest can carry
// a `[build-dependencies] vergen = "=9.0.6"` pin.
//
// librespot-core 0.8's build script uses both `vergen` and `vergen-gitcl`, which
// must share one `vergen-lib`. If `vergen` resolves to 9.1+ it pulls in
// `vergen-lib 9.1`, while `vergen-gitcl 1.0` still pins `vergen-lib 0.1`, and the
// two trait versions collide. Forcing `vergen` to 9.0.6 keeps them aligned.
// Needed because `cargo install` re-resolves and ignores Cargo.lock.
fn main() {}
