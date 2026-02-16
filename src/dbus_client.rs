//! Async D-Bus client used by the GUI to call daemon operations.

use std::sync::Arc;
use tracing::{debug, error, info};
use zbus::{Connection, Result as ZbusResult};

use crate::models::{
    Error, ExecutionResult, Profile, Result, DBUS_OBJECT_PATH, DBUS_SERVICE_NAME,
};

/// D-Bus client for the Network Manager daemon.
#[allow(dead_code)]
#[derive(Clone)]
pub struct DaemonClient {
    connection: Option<Arc<Connection>>,
}

#[allow(dead_code)]
impl DaemonClient {
    /// Create a new daemon client.
    pub fn new() -> Self {
        Self { connection: None }
    }

    /// Connect to the daemon.
    pub async fn connect(&mut self) -> Result<()> {
        match Connection::system().await {
            Ok(conn) => {
                debug!("Connected to system D-Bus");
                self.connection = Some(Arc::new(conn));
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to system D-Bus: {}", e);
                Err(Error::DbusConnectionFailed(e.to_string()))
            }
        }
    }

    /// Check if connected to the daemon.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Check if the daemon is running.
    pub async fn ping(&self) -> Result<bool> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<bool> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Manager"),
                "Ping",
                &(),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or(false));

        match result {
            Ok(pong) => Ok(pong),
            Err(e) => {
                debug!("Ping failed: {}", e);
                Ok(false)
            }
        }
    }

    /// List all profiles.
    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<String> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Profiles"),
                "List",
                &(),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or_default());

        match result {
            Ok(json) => {
                let profiles: Vec<Profile> = serde_json::from_str(&json)?;
                Ok(profiles)
            }
            Err(e) => Err(Error::NetworkManagerDbus(e.to_string())),
        }
    }

    /// Get a profile by ID.
    pub async fn get_profile(&self, id: &str) -> Result<Profile> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<String> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Profiles"),
                "Get",
                &(id,),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or_default());

        match result {
            Ok(json) => {
                let profile: Profile = serde_json::from_str(&json)?;
                Ok(profile)
            }
            Err(_e) => Err(Error::ProfileNotFound(id.to_string())),
        }
    }

    /// Activate a profile by ID.
    pub async fn activate_profile(&self, id: &str) -> Result<ExecutionResult> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        info!("Requesting profile activation: {}", id);

        let result: ZbusResult<String> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Manager"),
                "ActivateProfile",
                &(id,),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or_default());

        match result {
            Ok(json) => {
                let execution_result: ExecutionResult = serde_json::from_str(&json)?;
                Ok(execution_result)
            }
            Err(e) => {
                error!("Profile activation failed: {}", e);
                Err(Error::ActionFailed {
                    action: "ActivateProfile".to_string(),
                    reason: e.to_string(),
                })
            }
        }
    }

    /// Create a new profile.
    pub async fn create_profile(&self, profile: &Profile) -> Result<()> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let json = serde_json::to_string(profile)?;

        let result: ZbusResult<()> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Profiles"),
                "Create",
                &(json,),
            )
            .await
            .map(|_| ());

        result.map_err(|e| Error::Dbus(e.to_string()))
    }

    /// Update an existing profile.
    pub async fn update_profile(&self, profile: &Profile) -> Result<()> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let json = serde_json::to_string(profile)?;

        let result: ZbusResult<()> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Profiles"),
                "Update",
                &(profile.id().to_string(), json),
            )
            .await
            .map(|_| ());

        result.map_err(|e| Error::Dbus(e.to_string()))
    }

    /// Delete a profile.
    pub async fn delete_profile(&self, id: &str) -> Result<()> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<()> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Profiles"),
                "Delete",
                &(id,),
            )
            .await
            .map(|_| ());

        result.map_err(|e| Error::Dbus(e.to_string()))
    }

    /// Get the current active profile ID.
    pub async fn get_active_profile_id(&self) -> Result<Option<String>> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<String> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Status"),
                "GetActiveProfile",
                &(),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or_default());

        match result {
            Ok(id) if !id.is_empty() => Ok(Some(id)),
            Ok(_) => Ok(None),
            Err(e) => Err(Error::Dbus(e.to_string())),
        }
    }

    /// Get the daemon status.
    pub async fn get_status(&self) -> Result<DaemonStatus> {
        let conn = self.connection.as_ref().ok_or(Error::DaemonNotRunning)?;

        let result: ZbusResult<String> = conn
            .call_method(
                Some(DBUS_SERVICE_NAME),
                DBUS_OBJECT_PATH,
                Some("com.chrisdaggas.NetworkManager.Status"),
                "Get",
                &(),
            )
            .await
            .map(|reply| reply.body().deserialize().unwrap_or_default());

        match result {
            Ok(json) => {
                let status: DaemonStatus = serde_json::from_str(&json)?;
                Ok(status)
            }
            Err(e) => Err(Error::Dbus(e.to_string())),
        }
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Daemon status information.
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonStatus {
    /// Daemon version.
    pub version: String,
    /// Whether the daemon is healthy.
    pub healthy: bool,
    /// Currently active profile ID.
    pub active_profile_id: Option<String>,
    /// Auto-switch enabled.
    pub auto_switch_enabled: bool,
    /// Last error message.
    pub last_error: Option<String>,
}

impl Default for DaemonStatus {
    fn default() -> Self {
        Self {
            version: String::new(),
            healthy: false,
            active_profile_id: None,
            auto_switch_enabled: false,
            last_error: None,
        }
    }
}
