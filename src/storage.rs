// Network Manager - Local Storage
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Local data storage for the GUI application.
//!
//! Handles:
//! - Profile listing (from daemon or local cache)
//! - Application settings
//! - Log history
//!
//! This module uses RwLock for thread-safe access. Lock poisoning is handled
//! gracefully by recovering the inner value, as poison indicates a panic
//! in another thread but the data itself may still be valid.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::{error, info, warn};

use crate::models::{AppConfig, Profile, CONFIG_DIR_NAME};
use crate::services::ProfileEncryption;

/// A log entry with timestamp, level, and message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Local data store for the GUI application.
#[derive(Debug)]
pub struct DataStore {
    /// Configuration directory path.
    config_dir: PathBuf,
    /// Settings file path.
    settings_file: PathBuf,
    /// Local profile cache file.
    profiles_cache_file: PathBuf,
    /// Log file path.
    logs_file: PathBuf,

    /// In-memory profile cache (read from daemon).
    profiles: RwLock<HashMap<String, Profile>>,
    /// Application settings.
    settings: RwLock<AppConfig>,
    /// In-memory log entries (also persisted to disk).
    logs: RwLock<Vec<LogEntry>>,
}

impl DataStore {
    /// Create a new data store with default config directory.
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_DIR_NAME);
        Self::with_config_dir(config_dir)
    }

    /// Create a new data store with a specific config directory.
    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        // Create directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&config_dir) {
            error!("Failed to create config directory: {}", e);
        }
        // Set restrictive permissions on the config directory (0700)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700));
        }

        let store = Self {
            settings_file: config_dir.join("settings.toml"),
            profiles_cache_file: config_dir.join("profiles_cache.json"),
            logs_file: config_dir.join("logs.json"),
            config_dir,
            profiles: RwLock::new(HashMap::new()),
            settings: RwLock::new(AppConfig::default()),
            logs: RwLock::new(Vec::new()),
        };

        store.load_settings();
        store.load_logs();
        store
    }

    // ========================================================================
    // RwLock Helper Methods (handle poisoning gracefully)
    // ========================================================================

    /// Read from RwLock, recovering from poison if needed.
    fn read_lock<T, F, R>(&self, lock: &RwLock<T>, context: &str, reader: F) -> R
    where
        F: FnOnce(&T) -> R,
        T: Default,
        R: Default,
    {
        match lock.read() {
            Ok(guard) => reader(&*guard),
            Err(poisoned) => {
                warn!("RwLock poisoned reading {}, recovering", context);
                reader(&*poisoned.into_inner())
            }
        }
    }

    /// Write to RwLock, recovering from poison if needed.
    fn write_lock<T, F>(&self, lock: &RwLock<T>, context: &str, writer: F)
    where
        F: FnOnce(&mut T),
    {
        match lock.write() {
            Ok(mut guard) => writer(&mut *guard),
            Err(poisoned) => {
                warn!("RwLock poisoned writing {}, recovering", context);
                writer(&mut *poisoned.into_inner())
            }
        }
    }

    // ========================================================================
    // Settings
    // ========================================================================

    /// Load the application configuration.
    pub fn load_config() -> Option<AppConfig> {
        let config_dir = dirs::config_dir()?.join(CONFIG_DIR_NAME);
        let settings_file = config_dir.join("settings.toml");
        
        if settings_file.exists() {
            AppConfig::load_from_file(&settings_file).ok()
        } else {
            None
        }
    }

    /// Load settings from disk.
    fn load_settings(&self) {
        if self.settings_file.exists() {
            match AppConfig::load_from_file(&self.settings_file) {
                Ok(config) => {
                    self.write_lock(&self.settings, "settings", |s| {
                        *s = config;
                    });
                    info!("Loaded settings from {:?}", self.settings_file);
                }
                Err(e) => {
                    error!("Failed to load settings: {}", e);
                }
            }
        }
    }

    /// Save settings to disk.
    fn save_settings(&self) {
        let settings = self.settings();
        if let Err(e) = settings.save_to_file(&self.settings_file) {
            error!("Failed to save settings: {}", e);
        }
    }

    /// Get the current settings.
    pub fn settings(&self) -> AppConfig {
        self.read_lock(&self.settings, "settings", |s| s.clone())
    }

    /// Update settings.
    pub fn update_settings(&self, settings: AppConfig) {
        self.write_lock(&self.settings, "settings", |s| {
            *s = settings;
        });
        self.save_settings();
    }

    /// Save configuration.
    pub fn save_config(&self, config: &AppConfig) -> Result<(), crate::models::Error> {
        config.save_to_file(&self.settings_file)
    }

    // ========================================================================
    // Profiles
    // ========================================================================

    /// Get all profiles.
    pub fn profiles(&self) -> Vec<Profile> {
        self.read_lock(&self.profiles, "profiles", |p| {
            p.values().cloned().collect()
        })
    }

    /// Get a profile by ID.
    pub fn profile(&self, id: &str) -> Option<Profile> {
        self.read_lock(&self.profiles, "profiles", |p| p.get(id).cloned())
    }

    /// Update the local profile cache.
    pub fn update_profiles_cache(&self, profiles: Vec<Profile>) {
        self.write_lock(&self.profiles, "profiles", |cache| {
            cache.clear();
            for profile in profiles {
                cache.insert(profile.id().to_string(), profile);
            }
        });
        self.save_profiles_cache();
    }

    // ========================================================================
    // Profiles Cache
    // ========================================================================

    /// Load profiles cache from disk.
    pub fn load_profiles_cache(&self) {
        let settings = self.settings();
        
        // Check for encrypted file first
        let enc_file = self.profiles_cache_file.with_extension("json.enc");
        if enc_file.exists() {
            if let Some(ref key) = settings.encryption_key {
                match fs::read_to_string(&enc_file) {
                    Ok(encrypted) => {
                        let encryption = ProfileEncryption::with_key(key);
                        match encryption.decrypt_json::<Vec<Profile>>(&encrypted) {
                            Ok(profiles) => {
                                let profile_count = profiles.len();
                                self.write_lock(&self.profiles, "profiles_cache", |cache| {
                                    for profile in profiles {
                                        cache.insert(profile.id().to_string(), profile);
                                    }
                                });
                                info!("Loaded {} encrypted profiles from cache", profile_count);
                                return;
                            }
                            Err(e) => {
                                warn!("Failed to decrypt profiles: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read encrypted profiles: {}", e);
                    }
                }
            } else {
                warn!("Encrypted profiles file exists but no encryption key configured");
            }
        }
        
        // Fall back to unencrypted file
        if !self.profiles_cache_file.exists() {
            return;
        }

        match File::open(&self.profiles_cache_file) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader::<_, Vec<Profile>>(reader) {
                    Ok(profiles) => {
                        let profile_count = profiles.len();
                        self.write_lock(&self.profiles, "profiles_cache", |cache| {
                            for profile in profiles {
                                cache.insert(profile.id().to_string(), profile);
                            }
                        });
                        info!("Loaded {} profiles from cache", profile_count);
                    }
                    Err(e) => {
                        error!("Failed to parse profiles cache: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to open profiles cache: {}", e);
            }
        }
    }

    /// Save profiles cache to disk.
    fn save_profiles_cache(&self) {
        let profiles: Vec<Profile> = self.profiles();
        
        // Check if encryption is enabled
        let settings = self.settings();
        
        if settings.encrypt_profiles {
            if let Some(ref key) = settings.encryption_key {
                // Save encrypted
                let encryption = ProfileEncryption::with_key(key);
                match encryption.encrypt_json(&profiles) {
                    Ok(encrypted) => {
                        // Save to .enc file
                        let enc_file = self.profiles_cache_file.with_extension("json.enc");
                        if let Err(e) = fs::write(&enc_file, &encrypted) {
                            error!("Failed to write encrypted profiles: {}", e);
                        } else {
                            // Set restrictive permissions on encrypted file
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let _ = fs::set_permissions(&enc_file, fs::Permissions::from_mode(0o600));
                            }
                            // Remove unencrypted file if it exists
                            let _ = fs::remove_file(&self.profiles_cache_file);
                            info!("Saved {} encrypted profiles", profiles.len());
                        }
                        return;
                    }
                    Err(e) => {
                        warn!("Encryption failed, saving unencrypted: {:?}", e);
                    }
                }
            }
        }
        
        // Save unencrypted (default)
        match File::create(&self.profiles_cache_file) {
            Ok(file) => {
                // Set restrictive permissions on cache file
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&self.profiles_cache_file, fs::Permissions::from_mode(0o600));
                }
                let writer = BufWriter::new(file);
                if let Err(e) = serde_json::to_writer_pretty(writer, &profiles) {
                    error!("Failed to write profiles cache: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to create profiles cache file: {}", e);
            }
        }
    }

    // ========================================================================
    // Logs
    // ========================================================================

    /// Load logs from disk.
    fn load_logs(&self) {
        if !self.logs_file.exists() {
            return;
        }

        match File::open(&self.logs_file) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader::<_, Vec<LogEntry>>(reader) {
                    Ok(entries) => {
                        let entry_count = entries.len();
                        self.write_lock(&self.logs, "logs", |logs| {
                            *logs = entries;
                        });
                        info!("Loaded {} log entries from disk", entry_count);
                    }
                    Err(e) => {
                        error!("Failed to parse logs file: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to open logs file: {}", e);
            }
        }
    }

    /// Save logs to disk.
    fn save_logs(&self) {
        let logs = self.logs();
        
        match File::create(&self.logs_file) {
            Ok(file) => {
                // Set restrictive permissions on log file
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&self.logs_file, fs::Permissions::from_mode(0o600));
                }
                let writer = BufWriter::new(file);
                if let Err(e) = serde_json::to_writer_pretty(writer, &logs) {
                    error!("Failed to write logs file: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to create logs file: {}", e);
            }
        }
    }

    /// Get all log entries.
    pub fn logs(&self) -> Vec<LogEntry> {
        self.read_lock(&self.logs, "logs", |l| l.clone())
    }

    /// Append a log entry and save to disk.
    pub fn append_log(&self, level: &str, message: &str) {
        let now = chrono::Local::now();
        let entry = LogEntry {
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            level: level.to_uppercase(),
            message: message.to_string(),
        };
        
        let max_entries = self.settings().max_log_entries;
        
        self.write_lock(&self.logs, "logs", |logs| {
            logs.push(entry);
            
            // Keep only the last N entries based on config
            if logs.len() > max_entries {
                let drain_count = logs.len() - max_entries;
                logs.drain(0..drain_count);
            }
        });
        
        self.save_logs();
    }

    /// Clear all logs and save to disk.
    pub fn clear_logs(&self) {
        self.write_lock(&self.logs, "logs", |logs| {
            logs.clear();
        });
        self.save_logs();
    }

    /// Get the config directory path.
    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }
}

impl Default for DataStore {
    fn default() -> Self {
        Self::new()
    }
}
