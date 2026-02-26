# Kucheat

**CHZZK Live Stream Notification Tray App for Linux** — Get desktop notifications when your favorite streamers go live.

## Prerequisites

### System Dependencies

```bash
# Ubuntu / Debian
sudo apt install libdbus-1-dev pkg-config

# Fedora
sudo dnf install dbus-devel pkg-config

# Arch Linux
sudo pacman -S dbus pkg-config
```

### Rust

Requires Rust **1.85+** (edition 2024).

## Build

```bash
cargo build --release
```

## Install

```bash
make install
```

### Uninstall

```bash
make uninstall
```

## Configuration

A default config file is created on first run at `~/.config/kucheat/config.toml`:

```toml
[api]
# CHZZK Open API credentials
client_id = ""
client_secret = ""

[settings]
# Live check interval in seconds
check_interval_secs = 60
# Send a notification when a stream goes offline
notify_offline = false

# Channels to monitor
[[channels]]
id = "CHANNEL_ID"
name = "Streamer Name"
```

### CHZZK Open API Credentials

Register an application at the [CHZZK Developer Center](https://developers.chzzk.naver.com) and obtain a Client ID and Client Secret. The official API (`openapi.chzzk.naver.com`) is used for channel info lookups; the unofficial API is used for live status checks (as the official API does not expose per-channel live state).

If credentials are left empty, only the unofficial API will be used.

## Usage

### Channel Management

```bash
# Add a channel (name auto-resolved from API)
kucheat add <CHANNEL_ID>

# Add a channel with an explicit name
kucheat add <CHANNEL_ID> --name "Streamer Name"

# Remove a channel
kucheat remove <CHANNEL_ID>

# List monitored channels
kucheat list

# Check current live status
kucheat status
```

### Daemon (Systemd)

```bash
# Enable auto-start on login
systemctl --user enable kucheat

# Start the daemon
systemctl --user start kucheat

# Check status
systemctl --user status kucheat

# Stream logs
journalctl --user -u kucheat -f

# Stop the daemon
systemctl --user stop kucheat
```

When the daemon starts, it automatically spawns the system tray in the same process with shared in-memory state.

### Daemon (XDG Autostart)

If you want to run a program while using a desktop session, you can add a desktop file to `~/.config/autostart/` by using command `auto-launch`

```bash
# Enable auto launch
kucheat auto-launch install
```

### System Tray

```bash
# Manual standalone start (reads state from disk)
kucheat tray
```

Tray menu features:

- **Live channels** — shows stream title, viewer count, with sub-menu links to open the live stream or channel page in your browser
- **Offline channels** — shows a link to open the channel page

### Notifications

Desktop notifications include action buttons:

- **라이브 보기** — Opens the live stream in your default browser
- **채널 페이지** — Opens the channel page

## License

Distributed under the [LICENSE](LICENSE)

## AI-Generated Code Notice

Parts of this project were created with assistance from AI tools (e.g. large language models). All AI-assisted contributions were reviewed and adapted by maintainers before inclusion. If you need provenance for specific changes, please refer to the Git history and commit messages.