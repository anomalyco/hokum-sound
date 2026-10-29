# Hokum Sound

Audio control for Linux, by Anomaly. Volume, devices, Bluetooth profiles and
per-app audio on PipeWire, in one small window. One of the Hokum tools.

```sh
sound                 # open the window
sound --mock          # the window on fake devices (no PipeWire)
sound check           # problems in ~/.config/hokum-sound/config.kdl
sound update          # check for a new build
```

## Install

- Arch: `yay -S hokum-sound-bin`
- Debian / Ubuntu: `curl -fLo /tmp/hokum-sound.deb https://anoma.ly/release/hokum/sound/stable/linux-x64-deb && sudo apt install /tmp/hokum-sound.deb`
- Fedora: `curl -fLo /tmp/hokum-sound.rpm https://anoma.ly/release/hokum/sound/stable/linux-x64-rpm && sudo dnf install /tmp/hokum-sound.rpm`

(`linux-arm64-deb` / `linux-arm64-rpm` on aarch64.)

## Config

`~/.config/hokum-sound/config.kdl`:

```kdl
behaviour {
    appearance "system"      // system, light or dark
    check-for-updates #true  // hourly check-in as hokum/<channel>/<version>/sound
}
```

## Build

Needs Rust, clang and the PipeWire headers (`pipewire` on Arch,
`libpipewire-0.3-dev libspa-0.2-dev clang` on Ubuntu).

```sh
cargo build --release
target/release/sound --snap /tmp/sound.png --scene mock
```

Releases: [docs/releasing.md](docs/releasing.md). MIT licensed.
