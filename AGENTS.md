# Hokum Sound Agent Guide

Hokum Sound (`sound`) is a Linux-only audio control app — a pavucontrol
replacement — drawn with Rune on native PipeWire. One of the Hokum tools
(anomalyco/hokum is the notifications app): package `hokum-<tool>`, plain
binary name, config in `~/.config/hokum-<tool>/`.

## Speed

- Move fast and fix forward: merge once it compiles; wait only for checks
  that gate the merge.
- No fixed sleeps to wait on CI: use `gh run watch <id> --exit-status`.
- Never end your turn while still waiting on something.
- Dev releases never cancel each other: a running release always finishes,
  and pushes in between are skipped so only the newest pending run goes next
  (`cancel-in-progress: false` on Release's per-channel group). Never add
  `cancel-in-progress: true` or a cancel step to a build or publish workflow.

## Design

- Minimal. Every control has to earn its place.
- Progressive disclosure: the first level is what people touch daily (output
  volume and device, input, the apps playing). Profiles, codecs, ports,
  channel balance, virtual devices, rate/quantum and routing live one level
  down.
- Almost no copy: no explanations, labels only where an icon can't say it.
- At most about six controls on the first level.
- Review every UI change from screenshots (`sound --snap <png> --scene
  <name>`, on the mock backend) before calling it done.

## Layout

- `src/model.rs` — the contract: `State` snapshots, `Command`s, the
  `Audio` (UI side) / `Driver` (backend side) channel. Change it deliberately;
  both the backend and the UI build on it.
- `src/audio/` — the PipeWire backend (`audio::start() -> Audio`).
- `src/mock.rs` — fake devices and apps with animated levels; `--mock` and
  every headless scene.
- `src/ui/` — the Rune window; keep the `mock` scene (the release smoke test
  renders it).
- `src/update.rs` — the release check-in (`hokum/<channel>/<version>/sound`).
- `src/config.rs` — `~/.config/hokum-sound/config.kdl`.
- Releases: docs/releasing.md. `.github/` needs Dax.

## Building

Needs the PipeWire headers and clang for bindgen (`pipewire` on Arch;
`libpipewire-0.3-dev libspa-0.2-dev clang` on Ubuntu). `cargo test`, `cargo
build --release`, then `target/release/sound --snap /tmp/sound.png --scene
mock`.
