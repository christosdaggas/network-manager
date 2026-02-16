# Contributing to Network Manager Cosmic

Thank you for your interest in contributing to Network Manager Cosmic!

## Development Setup

### Prerequisites

- Rust 1.75 or later
- GTK4 development libraries (gtk4-devel)
- libadwaita development libraries (libadwaita-devel)
- NetworkManager development libraries
- D-Bus development libraries

**Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel NetworkManager-libnm-devel dbus-devel gettext-devel
```

**Ubuntu:**
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libnm-dev libdbus-1-dev gettext
```

### Development Tools

Install required Rust tools:
```bash
rustup component add rustfmt clippy
```

### Building

```bash
cargo build
```

### Running

```bash
cargo run
```

## Code Quality

### Before Submitting

Run these checks before submitting a PR:

```bash
# Format code
cargo fmt

# Run linter (with warnings as errors)
cargo clippy -- -D warnings

# Run tests
cargo test

# Check for security advisories (optional)
cargo audit
```

### CI Checks

The CI pipeline runs:
1. `cargo fmt --check` - Code formatting
2. `cargo clippy -- -D warnings` - Linter with strict mode
3. `cargo test` - All tests
4. `cargo build --release` - Release build

## Code Style

### Naming Conventions

- **Modules**: `snake_case` (e.g., `dashboard_page.rs`)
- **Types**: `PascalCase` (e.g., `Profile`, `NetworkAction`)
- **Functions**: `snake_case` verbs (e.g., `get_profile()`, `apply_action()`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `APP_ID`, `CONFIG_DIR_NAME`)

### Architecture

The codebase follows a layered architecture:

```
src/
├── main.rs              # Entry point
├── application.rs       # GTK Application setup, Tokio runtime
├── storage.rs           # Local data persistence
├── dbus_client.rs       # D-Bus client for daemon IPC
├── network_utils.rs     # Linux sysfs network detection
├── autostart.rs         # XDG autostart management
├── models/              # Domain models
│   ├── actions/         # Network, system, automation actions
│   ├── profile.rs       # Profile data model
│   ├── config.rs        # App configuration
│   ├── error.rs         # Error types
│   ├── rules.rs         # Auto-switch rules engine
│   └── validation.rs    # Input validation
└── ui/                  # GTK widgets and pages
    ├── main_window.rs   # Main application window
    ├── pages/           # Application pages
    └── widgets/         # Reusable UI components
```

### Key Rules

1. **No panics in production code**: Use `Result<T, E>` instead of `unwrap()`/`expect()`
2. **Handle lock poisoning**: Use `.unwrap_or_else(|e| e.into_inner())` for RwLock
3. **Graceful degradation**: Handle missing displays, D-Bus failures, etc.
4. **Async for I/O**: Use async/await for D-Bus operations

## Duplication Policy

Before adding new code, check for existing similar implementations:

1. Search for similar function names
2. Check if a trait could be extracted
3. Parameterize instead of copy-paste

### Duplication Checklist

After each change, ask:
- [ ] Does this logic exist elsewhere in the codebase?
- [ ] Could this be a shared function or trait?
- [ ] Are there similar UI patterns that could be components?

## D-Bus Integration

This application communicates with NetworkManager via D-Bus. Key considerations:

1. **Async operations**: All D-Bus calls should be async
2. **Error handling**: Handle connection failures gracefully
3. **Timeouts**: Use appropriate timeouts for operations
4. **Polkit**: Privilege escalation uses PolicyKit

## Testing

### Unit Tests

Add tests for:
- Domain logic in `models/`
- Validation functions
- Profile serialization/deserialization

### Integration Tests

For D-Bus integration, use mock services or test on real systems.

### Test Matrix

Test on these configurations before release:

| Distribution | Desktop | Display Server | Notes |
|-------------|---------|----------------|-------|
| Fedora 40+  | GNOME   | Wayland       | Primary target |
| Fedora 40+  | KDE     | Wayland       | Verify libadwaita styling |
| Ubuntu 24.04| GNOME   | Wayland       | Secondary target |
| Ubuntu 24.04| GNOME   | X11           | X11 fallback |
| COSMIC      | COSMIC  | Wayland       | Pop!_OS variant |

## Packaging

### Building Packages

```bash
# AppImage
./build/appimage/build-appimage.sh

# DEB package
cd build/deb && dpkg-buildpackage -b

# RPM package
rpmbuild -bb build/rpm/network-manager.spec

# Flatpak (requires flatpak-builder)
flatpak-builder --user --install --force-clean build-dir com.chrisdaggas.network-manager.yml
```

## Questions?

Open an issue for:
- Bug reports
- Feature requests
- Questions about the codebase
