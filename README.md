# Network Manager

<div align="center">

<img width="100%" height="auto" alt="network-manager-white" src="https://github.com/user-attachments/assets/7f1ba6a7-a613-49e3-ac3f-c14636e3d929" />

**A Linux-native network and system profile manager built with GTK4 and libadwaita.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![GTK4](https://img.shields.io/badge/GTK-4.14-green.svg)](https://www.gtk.org/)
[![libadwaita](https://img.shields.io/badge/libadwaita-1.6-purple.svg)](https://gnome.pages.gitlab.gnome.org/libadwaita/)

</div>

---

## What is it?

Network Manager lets you create, manage, and quickly switch between comprehensive network and system configuration profiles. Moving between home, office, and coffee-shop networks? Need different DNS, proxy, hostname, or firewall settings per environment? Create a profile for each and switch with one click.

## Features

### Network Configuration
- IPv4 and IPv6 (static, DHCP, link-local, disabled)
- DNS servers and search domains
- Static routes
- MTU configuration
- MAC address cloning / spoofing
- Wi-Fi connection switching
- VPN connect / disconnect via NetworkManager

### System Configuration
- Hostname (static and pretty)
- `/etc/hosts` entries
- Proxy settings (GNOME `gsettings`)
- Firewall zones (firewalld)
- Default printer (CUPS)
- Timezone
- Environment variables

### Automation
- Pre/post-activation scripts with optional sandboxing (bubblewrap, firejail)
- Program launch on profile activation
- Custom desktop notifications and sound alerts

### Auto-Switch Engine
Rule-based automatic profile activation, evaluated every 30 seconds:

| Condition | Description |
|-----------|-------------|
| Wi-Fi SSID | Match by exact name or regex pattern |
| Gateway MAC | Match the default gateway's MAC address |
| Interface state | Up, down, carrier, no-carrier |
| Ping target | Reachability check with timeout |
| Time window | Day-of-week and time range |
| Network available | Any connection active |
| NOT | Negate any condition |

Rules can be combined with **AND** / **OR** logic and prioritised per profile.

### Security
- Profile encryption with AES-256-GCM (Argon2id key derivation)
- Script sandboxing via bubblewrap or firejail
- Strict file permissions (0600 config/cache, 0700 config directory)
- Key material zeroed from memory on drop
- Input validation on all user-supplied command arguments
- Polkit policy and D-Bus daemon for privilege separation

### Other
- Dashboard with live network status overview
- Automatic update check (GitHub Releases)
- System tray integration
- Connection watchdog with configurable actions (notify, reconnect, switch profile, restart NetworkManager)
- Profile scheduling (time-based activation)
- Hotkey binding for quick profile switching
- Theme selector (system, light, dark)
- Localization: English, Greek, German, Spanish, French, Italian, Portuguese, Russian, Chinese, Arabic, Hindi

## Requirements

### Build Dependencies

- Rust 1.75+ with Cargo
- GTK4 ≥ 4.14 development files
- libadwaita ≥ 1.5 development files
- GLib ≥ 2.76 development files
- pkg-config
- gettext

### Runtime Dependencies

- GTK4 ≥ 4.14
- libadwaita ≥ 1.5
- NetworkManager
- Optional: bubblewrap or firejail (for script sandboxing)

### Installing Dependencies

**Fedora / RHEL:**
```bash
sudo dnf install gtk4-devel libadwaita-devel glib2-devel gettext-devel
```

**Ubuntu / Debian:**
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libglib2.0-dev gettext
```

**Arch Linux:**
```bash
sudo pacman -S gtk4 libadwaita glib2 gettext
```

## Building

```bash
git clone https://github.com/christosdaggas/network-manager.git
cd network-manager

cargo build --release
```

The binary is at `target/release/network-manager`.

### Installing

```bash
# Binary
sudo install -Dm755 target/release/network-manager /usr/local/bin/

# Desktop entry
sudo install -Dm644 data/com.chrisdaggas.network-manager.desktop \
    /usr/share/applications/com.chrisdaggas.network-manager.desktop

# AppStream metainfo
sudo install -Dm644 data/com.chrisdaggas.network-manager.metainfo.xml \
    /usr/share/metainfo/com.chrisdaggas.network-manager.metainfo.xml

# Icon
sudo install -Dm644 data/icons/hicolor/scalable/apps/com.chrisdaggas.network-manager.svg \
    /usr/share/icons/hicolor/scalable/apps/com.chrisdaggas.network-manager.svg

# Polkit policy (optional — for D-Bus daemon)
sudo install -Dm644 data/polkit/com.chrisdaggas.network-manager.policy \
    /usr/share/polkit-1/actions/com.chrisdaggas.network-manager.policy
```

### Packages

Pre-built packages can be generated with the packaging script:

```bash
# RPM, DEB, and AppImage
./packaging/build-packages.sh
```

The script auto-detects architecture (`x86_64`, `aarch64`) and outputs to `dist/`.

## Usage

Launch from your application menu or run:

```bash
network-manager
```

### Quick Start

1. Open the app and navigate to **Profiles**
2. Click **+ New Profile**
3. Add network actions (IPv4, DNS, routes, etc.) and system actions (hostname, proxy, etc.)
4. Click **Apply** to activate the profile
5. Optionally configure auto-switch rules to activate profiles automatically

## Project Structure

```
├── Cargo.toml                  # Project manifest
├── build.rs                    # GResource compilation
├── src/
│   ├── main.rs                 # Entry point
│   ├── application.rs          # Adwaita application lifecycle
│   ├── storage.rs              # DataStore (profiles, config, logs)
│   ├── dbus_client.rs          # D-Bus client for daemon
│   ├── network_utils.rs        # Network helper functions
│   ├── scheduler.rs            # Time-based profile scheduler
│   ├── tray.rs                 # System tray integration
│   ├── version_check.rs        # GitHub release checker
│   ├── autostart.rs            # XDG autostart support
│   ├── models/                 # Data models (Profile, Config, Actions, Rules)
│   ├── services/               # Background services (autoswitch, encryption,
│   │                           #   sandbox, watchdog)
│   └── ui/                     # GTK4/libadwaita UI
│       ├── main_window.rs
│       ├── pages/              # Dashboard, profiles, settings, logs, etc.
│       └── widgets/            # Reusable UI components
├── data/                       # Desktop entry, icons, metainfo, CSS,
│                               #   Polkit, D-Bus, systemd configs
├── po/                         # Gettext translations (11 languages)
├── packaging/                  # RPM spec, DEB control, AppImage config
└── scripts/                    # Dev scripts (lint, clean, build)
```

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m 'Add my feature'`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a Pull Request

## License

MIT — see [LICENSE](LICENSE) for details.

## Author

**Christos A. Daggas** — [chrisdaggas.com](https://chrisdaggas.com) · [GitHub](https://github.com/christosdaggas)

---

<div align="center">
Made with ❤️ for the Linux community
</div>
