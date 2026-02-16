// Network Manager - Background Mode Support
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Background mode support via D-Bus.
//!
//! This provides background mode where the application continues
//! running when the window is closed. The app can be reopened via
//! D-Bus activation or by running the application again.
//! 
//! Note: Traditional system tray icons require additional dependencies
//! (libappindicator, etc.) that may not be available on all systems.
//! This module provides a D-Bus-based alternative for background control.

use std::sync::mpsc::{channel, Receiver, Sender};
use zbus::{interface, Connection};

/// Commands that can be sent from external sources to the main application.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Show the main window
    ShowWindow,
    /// Apply a profile by its ID
    ApplyProfile(String),
    /// Quit the application
    Quit,
}

/// D-Bus interface for controlling the application from external sources.
/// This allows the app to be controlled via D-Bus when running in background.
pub struct ApplicationInterface {
    command_tx: Sender<TrayCommand>,
}

#[interface(name = "com.chrisdaggas.NetworkManager.Application")]
impl ApplicationInterface {
    /// Show the main application window.
    async fn show(&self) {
        let _ = self.command_tx.send(TrayCommand::ShowWindow);
    }
    
    /// Apply a profile by ID.
    async fn apply_profile(&self, profile_id: &str) {
        let _ = self.command_tx.send(TrayCommand::ApplyProfile(profile_id.to_string()));
    }
    
    /// Quit the application.
    async fn quit(&self) {
        let _ = self.command_tx.send(TrayCommand::Quit);
    }
}

/// Handle for the running background service.
pub struct TrayHandle {
    #[allow(dead_code)]
    connection: Connection,
}

impl TrayHandle {
    /// Update the active profile (no-op for D-Bus backend, but keeps API consistent).
    pub fn set_active_profile(&self, _name: Option<String>) {
        // D-Bus backend doesn't maintain state - this is a no-op
    }
    
    /// Update the list of profiles (no-op for D-Bus backend, but keeps API consistent).
    pub fn set_profiles(&self, _profiles: Vec<(String, String)>) {
        // D-Bus backend doesn't maintain state - this is a no-op
    }
}

/// Start the background mode D-Bus service.
/// 
/// This exposes the application on D-Bus so it can be controlled
/// when running in background mode.
/// 
/// Returns a handle and a receiver for commands.
pub fn start_tray() -> Option<(TrayHandle, Receiver<TrayCommand>)> {
    let (tx, rx) = channel();
    
    let interface = ApplicationInterface { command_tx: tx };
    
    // Run the async setup on a blocking thread
    let result = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        
        rt.block_on(async {
            let connection = Connection::session().await.ok()?;
            
            // Try to request the well-known name
            let _ = connection
                .request_name("com.chrisdaggas.NetworkManager.Background")
                .await;
            
            // Serve the interface
            connection
                .object_server()
                .at("/com/chrisdaggas/NetworkManager", interface)
                .await
                .ok()?;
            
            tracing::info!("Background D-Bus service started");
            
            Some(connection)
        })
    }).join().ok()??;
    
    Some((TrayHandle { connection: result }, rx))
}

/// Try to activate an already-running instance of the application.
/// 
/// Returns true if an existing instance was found and activated.
#[allow(dead_code)]
pub fn activate_existing_instance() -> bool {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return false,
    };
    
    rt.block_on(async {
        let connection = match Connection::session().await {
            Ok(conn) => conn,
            Err(_) => return false,
        };
        
        // Try to call Show on an existing instance
        let result = connection
            .call_method(
                Some("com.chrisdaggas.NetworkManager.Background"),
                "/com/chrisdaggas/NetworkManager",
                Some("com.chrisdaggas.NetworkManager.Application"),
                "Show",
                &(),
            )
            .await;
        
        result.is_ok()
    })
}
