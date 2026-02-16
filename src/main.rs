// Network Manager - Main Entry Point
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! # Network Manager
//!
//! A GTK4/libadwaita network and system profile manager for Linux.
//!
//! This is the main entry point for the GUI application.

use gtk4::prelude::*;
use gtk4::glib;
#[cfg(feature = "gresource")]
use gtk4::gio;
use std::env;

mod application;
mod autostart;
mod dbus_client;
mod models;
mod network_utils;
mod scheduler;
mod services;
mod storage;
mod tray;
mod ui;
mod version_check;

use application::Application;

/// Application ID for GNOME/Freedesktop.
pub const APP_ID: &str = models::APP_ID;

/// Human-readable application name.
pub const APP_NAME: &str = "Network Manager";

/// Application version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Gettext domain for translations.
pub const GETTEXT_DOMAIN: &str = "network-manager";

/// Print version information and exit.
fn print_version() {
    println!("{} {}", APP_NAME, VERSION);
    println!("Copyright (C) 2026 Christos A. Daggas");
    println!("License: MIT");
    println!();
    println!("A GTK4/libadwaita network and system profile manager for Linux.");
}

/// Print help information and exit.
fn print_help() {
    println!("Usage: {} [OPTIONS]", env::args().next().unwrap_or_else(|| "network-manager".to_string()));
    println!();
    println!("A GTK4/libadwaita network and system profile manager for Linux.");
    println!();
    println!("Options:");
    println!("  -h, --help       Show this help message and exit");
    println!("  -v, --version    Show version information and exit");
    println!("  -m, --minimized  Start minimized to system tray");
    println!("  -d, --debug      Enable debug logging");
    println!();
    println!("Environment variables:");
    println!("  RUST_LOG         Set log level (trace, debug, info, warn, error)");
    println!();
    println!("Report bugs to: https://github.com/christosdaggas/network-manager/issues");
}

/// Initialize internationalization (gettext).
fn setup_i18n() {
    use gettextrs::{LocaleCategory, setlocale, bindtextdomain, textdomain};
    
    // Set locale from environment
    setlocale(LocaleCategory::LcAll, "");
    
    // Try to find locale directory
    let locale_dirs = [
        "/usr/share/locale",
        "/usr/local/share/locale",
        concat!(env!("CARGO_MANIFEST_DIR"), "/po"),
    ];
    
    for dir in &locale_dirs {
        if std::path::Path::new(dir).exists() {
            if let Err(e) = bindtextdomain(GETTEXT_DOMAIN, *dir) {
                tracing::warn!("Failed to bind textdomain to {}: {}", dir, e);
            } else {
                tracing::debug!("Bound textdomain to {}", dir);
                break;
            }
        }
    }
    
    if let Err(e) = textdomain(GETTEXT_DOMAIN) {
        tracing::warn!("Failed to set textdomain: {}", e);
    }
}

fn main() -> glib::ExitCode {
    // Parse command-line arguments before GTK initialization
    let args: Vec<String> = env::args().collect();
    let mut start_minimized = false;
    let mut debug_mode = false;
    
    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return glib::ExitCode::SUCCESS;
            }
            "-v" | "--version" => {
                print_version();
                return glib::ExitCode::SUCCESS;
            }
            "-m" | "--minimized" => {
                start_minimized = true;
            }
            "-d" | "--debug" => {
                debug_mode = true;
            }
            _ => {
                if arg.starts_with('-') {
                    eprintln!("Unknown option: {}", arg);
                    eprintln!("Try '--help' for more information.");
                    return glib::ExitCode::FAILURE;
                }
            }
        }
    }
    
    // Set the program name to match StartupWMClass in the .desktop file
    glib::set_prgname(Some(APP_ID));
    glib::set_application_name(APP_NAME);

    // Initialize logging with appropriate level
    let log_level = if debug_mode {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.into()),
        )
        .init();

    tracing::info!("Starting {} v{}", APP_NAME, VERSION);
    if start_minimized {
        tracing::info!("Starting in minimized mode");
    }

    // Initialize internationalization
    setup_i18n();

    // Initialize GTK and Libadwaita
    if let Err(e) = libadwaita::init() {
        eprintln!("Failed to initialize libadwaita: {}", e);
        eprintln!("This application requires a graphical environment.");
        return glib::ExitCode::FAILURE;
    }

    // Add the data/icons directory to the icon theme search path for development
    // Handle headless environments gracefully (e.g., SSH sessions without display)
    let display = match gtk4::gdk::Display::default() {
        Some(d) => d,
        None => {
            eprintln!("No display found. This application requires a graphical environment.");
            eprintln!("If running via SSH, ensure X11 forwarding or Wayland socket is available.");
            return glib::ExitCode::FAILURE;
        }
    };
    let icon_theme = gtk4::IconTheme::for_display(&display);
    
    // Try to find the icons directory relative to the executable or in common locations
    let possible_paths = [
        // Development: relative to current working directory
        std::path::PathBuf::from("data/icons"),
        // Development: relative to cargo manifest
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/icons"),
        // Installed: standard locations
        std::path::PathBuf::from("/usr/share/icons"),
        std::path::PathBuf::from("/usr/local/share/icons"),
        dirs::data_dir().unwrap_or_default().join("icons"),
    ];
    
    for path in &possible_paths {
        if path.exists() {
            icon_theme.add_search_path(path);
            tracing::debug!("Added icon search path: {:?}", path);
        }
    }

    // Register resources (if gresource available)
    #[cfg(feature = "gresource")]
    {
        let resource_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/network-manager.gresource"));
        let resource_data = glib::Bytes::from_static(resource_bytes);
        if let Ok(resource) = gio::Resource::from_data(&resource_data) {
            gio::resources_register(&resource);
        }
    }

    // Create and run the application
    let app = Application::new();
    app.run()
}

/// Helper macro for gettext translations.
#[macro_export]
macro_rules! i18n {
    ($s:expr) => {
        gettext_rs::gettext($s)
    };
}

/// Helper macro for ngettext (plurals).
#[macro_export]
macro_rules! ni18n {
    ($singular:expr, $plural:expr, $n:expr) => {
        gettext_rs::ngettext($singular, $plural, $n as u32)
    };
}

/// Helper macro for pgettext (context-aware translations).
#[macro_export]
macro_rules! pi18n {
    ($context:expr, $msgid:expr) => {
        gettext_rs::pgettext($context, $msgid)
    };
}
