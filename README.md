# hermes-cosmic

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](Cargo.toml)
[![Built with libcosmic](https://img.shields.io/badge/COSMIC-libcosmic-63b3ed)](https://github.com/pop-os/libcosmic)

A native **COSMIC™ Desktop Panel Applet** (built with `libcosmic`) for monitoring and controlling a local [Hermes Agent](https://github.com/NousResearch/hermes-agent) messaging gateway (`hermes-gateway.service`).

## Features

- **Minimalist Icon-Only Panel Indicator**: Displays an **outline** Hermes icon when `hermes-gateway.service` is stopped and automatically switches to a **filled** icon when the gateway is running—with dark/light theme adaptation and zero hover tooltip clutter.
- **Live Resource Telemetry & Uptime**:
  - Shows **Memory**, instantaneous **CPU %** (computed from systemd `CPUUsageNSec` deltas across polls), **PID**, and formatted uptime in the popup header (`Online · up ...`).
  - Auto-refreshes every 5 seconds with an on-demand refresh button in the header.
- **Platform Logos**: Displays a compact single-line `Platform` card with brand logos for connected messaging platforms parsed from `hermes logs gateway` (Telegram, Discord, WhatsApp, Weixin, Signal, Matrix, Slack, plus a generic fallback).
- **Gateway Profiles**: Lists active Hermes gateway profiles and PIDs from `hermes gateway list`.
- **Quick Actions & Service Controls**:
  - **Launch / Open Dashboard**: Checks `hermes dashboard --status`; opens `http://127.0.0.1:9119` in your browser if already running, or starts `hermes dashboard`.
  - **Quick Chat**: Launches `hermes chat` in `$TERMINAL` (or falls back to `ghostty`, `kitty`, `foot`, `alacritty`, `wezterm`, `gnome-terminal`, `konsole`, `xfce4-terminal`, or `xterm`).
  - **Start / Stop / Restart**: Controls `hermes-gateway.service` via `systemctl --user`.

## Prerequisites

- **COSMIC Desktop** (`cosmic-panel`)
- **Hermes Agent** installed with the `hermes-gateway.service` systemd user unit (`hermes gateway install`)
- **Rust toolchain** (`cargo` & `rustc` via [rustup.rs](https://rustup.rs) or your package manager) and Wayland / `xkbcommon` development libraries:
  - **Arch Linux**: `sudo pacman -S rust libxkbcommon wayland pkgconf`
  - **Pop!\_OS / Ubuntu / Debian**: `sudo apt install cargo pkg-config libwayland-dev libxkbcommon-dev`
  - **Fedora**: `sudo dnf install cargo pkgconf-pkg-config wayland-devel libxkbcommon-devel`

## Installation

### User Install (`~/.local`, no `sudo` required)

```bash
git clone https://github.com/ashrafpy/hermes-cosmic.git
cd hermes-cosmic
./install.sh
```

This builds the release binary, installs the executable, `.desktop` entry, and symbolic icons (outline + active filled) into `~/.local/`, and restarts any running `cosmic-applet-hermes` instance.

To add the applet to your COSMIC Panel:
1. Open **COSMIC Settings → Desktop → Panel → Configure Panel Applets**
2. Click **+ Add Applet** and select **Hermes Gateway**

### System-Wide Install (`just` or `make`)

```bash
just build
sudo just install
# or: make build && sudo make install
```

## Uninstallation

```bash
./uninstall.sh
```

*(If installed system-wide, run `sudo just uninstall` or `sudo make uninstall`.)*

## Development

```bash
cargo check
cargo test
cargo build --release
```

## License

Licensed under the [MIT License](Cargo.toml).
