// Network Manager - Settings Page
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Application settings page.

use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::glib;
use gtk4::gio;
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use tracing::info;

use crate::models::config::ThemePreference;
use crate::models::{CONFIG_DIR_NAME, Profile, ScheduleEntry, HotkeyEntry, SandboxMode, WatchdogAction, WatchdogConfig};
use crate::ui::MainWindow;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SettingsPage {
        pub theme_row: RefCell<Option<adw::ComboRow>>,
        pub auto_switch_row: RefCell<Option<adw::SwitchRow>>,
        pub auto_switch_interval: RefCell<Option<adw::SpinRow>>,
        pub notifications_row: RefCell<Option<adw::SwitchRow>>,
        pub start_minimized_row: RefCell<Option<adw::SwitchRow>>,
        // Scheduling
        pub scheduling_enabled_row: RefCell<Option<adw::SwitchRow>>,
        // Watchdog
        pub watchdog_enabled_row: RefCell<Option<adw::SwitchRow>>,
        pub watchdog_interval_row: RefCell<Option<adw::SpinRow>>,
        pub watchdog_target_row: RefCell<Option<adw::EntryRow>>,
        pub watchdog_threshold_row: RefCell<Option<adw::SpinRow>>,
        pub watchdog_action_row: RefCell<Option<adw::ComboRow>>,
        // Security
        pub sandbox_row: RefCell<Option<adw::ComboRow>>,
        pub encryption_row: RefCell<Option<adw::SwitchRow>>,
        pub encryption_key_row: RefCell<Option<adw::PasswordEntryRow>>,
        // Hotkeys
        pub hotkeys_enabled_row: RefCell<Option<adw::SwitchRow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsPage {
        const NAME: &'static str = "CdNetworkManagerSettingsPage";
        type Type = super::SettingsPage;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for SettingsPage {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_ui();
        }
    }

    impl WidgetImpl for SettingsPage {}
    impl gtk::subclass::prelude::BoxImpl for SettingsPage {}
}

glib::wrapper! {
    pub struct SettingsPage(ObjectSubclass<imp::SettingsPage>)
        @extends gtk::Widget, gtk::Box;
}

impl SettingsPage {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("orientation", gtk::Orientation::Vertical)
            .property("spacing", 0)
            .build()
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // Scrolled container
        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        self.append(&scrolled);

        // Content box with margins
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .hexpand(true)
            .build();
        scrolled.set_child(Some(&content));

        // Appearance group
        let appearance_group = adw::PreferencesGroup::builder()
            .title("Appearance")
            .build();

        let theme_model = gtk::StringList::new(&["System", "Light", "Dark"]);
        let theme_row = adw::ComboRow::builder()
            .title("Color Scheme")
            .subtitle("Select the application color scheme")
            .model(&theme_model)
            .build();
        appearance_group.add(&theme_row);
        *imp.theme_row.borrow_mut() = Some(theme_row);

        content.append(&appearance_group);

        // Auto-Switch group
        let auto_switch_group = adw::PreferencesGroup::builder()
            .title("Auto-Switch")
            .description("Automatically activate profiles based on conditions")
            .build();

        let auto_switch_row = adw::SwitchRow::builder()
            .title("Enable Auto-Switch")
            .subtitle("Activate profiles when conditions match")
            .active(true)
            .build();
        auto_switch_group.add(&auto_switch_row);
        *imp.auto_switch_row.borrow_mut() = Some(auto_switch_row);

        let auto_switch_interval = adw::SpinRow::builder()
            .title("Check Interval")
            .subtitle("Seconds between condition checks")
            .adjustment(&gtk::Adjustment::new(30.0, 5.0, 300.0, 5.0, 30.0, 0.0))
            .build();
        auto_switch_group.add(&auto_switch_interval);
        *imp.auto_switch_interval.borrow_mut() = Some(auto_switch_interval);

        // Manage rules row
        let manage_rules_row = adw::ActionRow::builder()
            .title("Auto-Switch Rules")
            .subtitle("Configure conditions for automatic profile switching")
            .activatable(true)
            .build();
        let rules_icon = gtk::Image::from_icon_name("view-list-symbolic");
        manage_rules_row.add_suffix(&rules_icon);
        let rules_arrow = gtk::Image::from_icon_name("go-next-symbolic");
        manage_rules_row.add_suffix(&rules_arrow);
        auto_switch_group.add(&manage_rules_row);

        // Connect manage rules
        let this_for_rules = self.downgrade();
        manage_rules_row.connect_activated(move |_| {
            if let Some(this) = this_for_rules.upgrade() {
                this.show_manage_rules_dialog();
            }
        });

        content.append(&auto_switch_group);

        // Behavior group
        let behavior_group = adw::PreferencesGroup::builder()
            .title("Behavior")
            .build();

        let notifications_row = adw::SwitchRow::builder()
            .title("Desktop Notifications")
            .subtitle("Show notifications for profile changes")
            .active(true)
            .build();
        behavior_group.add(&notifications_row);
        *imp.notifications_row.borrow_mut() = Some(notifications_row);

        let start_minimized_row = adw::SwitchRow::builder()
            .title("Start Minimized")
            .subtitle("Minimize to system tray on startup")
            .active(false)
            .build();
        behavior_group.add(&start_minimized_row);
        *imp.start_minimized_row.borrow_mut() = Some(start_minimized_row);

        content.append(&behavior_group);

        // Storage group
        let storage_group = adw::PreferencesGroup::builder()
            .title("Storage")
            .build();

        let profiles_dir = Self::get_profiles_dir();
        let profiles_dir_row = adw::ActionRow::builder()
            .title("Profiles Directory")
            .subtitle(profiles_dir.to_string_lossy().as_ref())
            .build();
        let open_folder_btn = gtk::Button::from_icon_name("folder-symbolic");
        open_folder_btn.set_valign(gtk::Align::Center);
        open_folder_btn.set_tooltip_text(Some("Open folder"));
        profiles_dir_row.add_suffix(&open_folder_btn);
        storage_group.add(&profiles_dir_row);

        // Connect profiles folder button
        let profiles_dir_clone = profiles_dir.clone();
        open_folder_btn.connect_clicked(move |_| {
            // Ensure directory exists before opening
            if let Err(e) = std::fs::create_dir_all(&profiles_dir_clone) {
                tracing::error!("Failed to create profiles directory: {}", e);
                return;
            }
            let uri = format!("file://{}", profiles_dir_clone.display());
            if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
                tracing::error!("Failed to open profiles directory: {}", e);
            }
        });

        let logs_dir = Self::get_logs_dir();
        let logs_file = logs_dir.join("logs.json");
        let logs_dir_row = adw::ActionRow::builder()
            .title("Logs File")
            .subtitle(logs_file.to_string_lossy().as_ref())
            .build();
        let open_logs_btn = gtk::Button::from_icon_name("folder-symbolic");
        open_logs_btn.set_valign(gtk::Align::Center);
        open_logs_btn.set_tooltip_text(Some("Open folder"));
        logs_dir_row.add_suffix(&open_logs_btn);
        storage_group.add(&logs_dir_row);

        // Connect logs folder button
        let logs_dir_clone = logs_dir.clone();
        open_logs_btn.connect_clicked(move |_| {
            // Ensure directory exists before opening
            if let Err(e) = std::fs::create_dir_all(&logs_dir_clone) {
                tracing::error!("Failed to create logs directory: {}", e);
                return;
            }
            let uri = format!("file://{}", logs_dir_clone.display());
            if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>) {
                tracing::error!("Failed to open logs directory: {}", e);
            }
        });

        content.append(&storage_group);

        // Backup & Restore group
        let backup_group = adw::PreferencesGroup::builder()
            .title("Backup &amp; Restore")
            .build();

        let export_row = adw::ActionRow::builder()
            .title("Export Profiles")
            .subtitle("Save all profiles to a backup file")
            .build();
        let export_icon = gtk::Image::from_icon_name("document-save-symbolic");
        export_row.add_suffix(&export_icon);
        let export_btn = gtk::Button::from_icon_name("go-next-symbolic");
        export_btn.set_valign(gtk::Align::Center);
        export_btn.add_css_class("flat");
        export_row.add_suffix(&export_btn);
        backup_group.add(&export_row);

        let import_row = adw::ActionRow::builder()
            .title("Import Profiles")
            .subtitle("Restore profiles from a backup file")
            .build();
        let import_icon = gtk::Image::from_icon_name("document-open-symbolic");
        import_row.add_suffix(&import_icon);
        let import_btn = gtk::Button::from_icon_name("go-next-symbolic");
        import_btn.set_valign(gtk::Align::Center);
        import_btn.add_css_class("flat");
        import_row.add_suffix(&import_btn);
        backup_group.add(&import_row);

        // Connect export profiles button
        let cache_file_for_export = Self::get_profiles_cache_file();
        let this_for_export = self.downgrade();
        export_btn.connect_clicked(move |_| {
            if let Some(this) = this_for_export.upgrade() {
                Self::show_export_dialog(&this, &cache_file_for_export);
            }
        });

        // Connect import profiles button
        let cache_file_for_import = Self::get_profiles_cache_file();
        let this_for_import = self.downgrade();
        import_btn.connect_clicked(move |_| {
            if let Some(this) = this_for_import.upgrade() {
                Self::show_import_dialog(&this, &cache_file_for_import);
            }
        });

        content.append(&backup_group);

        // Scheduling group
        let scheduling_group = adw::PreferencesGroup::builder()
            .title("Scheduling")
            .description("Automatically activate profiles at specific times")
            .build();

        let scheduling_enabled_row = adw::SwitchRow::builder()
            .title("Enable Scheduled Activation")
            .subtitle("Automatically switch profiles based on schedule")
            .active(false)
            .build();
        scheduling_group.add(&scheduling_enabled_row);
        *imp.scheduling_enabled_row.borrow_mut() = Some(scheduling_enabled_row.clone());

        // Connect scheduling enabled change
        let this_for_sched_save = self.downgrade();
        scheduling_enabled_row.connect_active_notify(move |row| {
            if let Some(this) = this_for_sched_save.upgrade() {
                this.save_scheduling_enabled(row.is_active());
            }
        });

        let manage_schedules_row = adw::ActionRow::builder()
            .title("Manage Schedules")
            .subtitle("Configure time-based profile activations")
            .activatable(true)
            .build();
        let schedule_icon = gtk::Image::from_icon_name("alarm-symbolic");
        manage_schedules_row.add_suffix(&schedule_icon);
        let schedule_arrow = gtk::Image::from_icon_name("go-next-symbolic");
        manage_schedules_row.add_suffix(&schedule_arrow);
        scheduling_group.add(&manage_schedules_row);

        // Connect manage schedules click - will show schedule management dialog
        let this_for_schedules = self.downgrade();
        manage_schedules_row.connect_activated(move |_| {
            if let Some(this) = this_for_schedules.upgrade() {
                this.show_manage_schedules_dialog();
            }
        });

        content.append(&scheduling_group);

        // Watchdog group
        let watchdog_group = adw::PreferencesGroup::builder()
            .title("Connection Watchdog")
            .description("Monitor network connectivity and take action on failures")
            .build();

        let watchdog_enabled_row = adw::SwitchRow::builder()
            .title("Enable Watchdog")
            .subtitle("Periodically check network connectivity")
            .active(false)
            .build();
        watchdog_group.add(&watchdog_enabled_row);
        *imp.watchdog_enabled_row.borrow_mut() = Some(watchdog_enabled_row.clone());

        let watchdog_interval_row = adw::SpinRow::builder()
            .title("Check Interval")
            .subtitle("Seconds between connectivity checks")
            .adjustment(&gtk::Adjustment::new(30.0, 5.0, 300.0, 5.0, 30.0, 0.0))
            .build();
        watchdog_group.add(&watchdog_interval_row);
        *imp.watchdog_interval_row.borrow_mut() = Some(watchdog_interval_row.clone());

        let watchdog_target_row = adw::EntryRow::builder()
            .title("Ping Target")
            .text("8.8.8.8")
            .build();
        watchdog_group.add(&watchdog_target_row);
        *imp.watchdog_target_row.borrow_mut() = Some(watchdog_target_row.clone());

        let watchdog_threshold_row = adw::SpinRow::builder()
            .title("Failure Threshold")
            .subtitle("Failed checks before taking action")
            .adjustment(&gtk::Adjustment::new(3.0, 1.0, 10.0, 1.0, 1.0, 0.0))
            .build();
        watchdog_group.add(&watchdog_threshold_row);
        *imp.watchdog_threshold_row.borrow_mut() = Some(watchdog_threshold_row.clone());

        let action_model = gtk::StringList::new(&["Notify", "Reconnect", "Switch Profile", "Restart NetworkManager"]);
        let watchdog_action_row = adw::ComboRow::builder()
            .title("Failure Action")
            .subtitle("Action when connectivity is lost")
            .model(&action_model)
            .build();
        watchdog_group.add(&watchdog_action_row);
        *imp.watchdog_action_row.borrow_mut() = Some(watchdog_action_row.clone());

        // Connect watchdog save handlers
        let this_for_wd = self.downgrade();
        watchdog_enabled_row.connect_active_notify(move |_| {
            if let Some(this) = this_for_wd.upgrade() {
                this.save_watchdog_settings();
            }
        });
        let this_for_wd2 = self.downgrade();
        watchdog_interval_row.connect_value_notify(move |_| {
            if let Some(this) = this_for_wd2.upgrade() {
                this.save_watchdog_settings();
            }
        });
        let this_for_wd3 = self.downgrade();
        watchdog_threshold_row.connect_value_notify(move |_| {
            if let Some(this) = this_for_wd3.upgrade() {
                this.save_watchdog_settings();
            }
        });
        let this_for_wd4 = self.downgrade();
        watchdog_action_row.connect_selected_notify(move |_| {
            if let Some(this) = this_for_wd4.upgrade() {
                this.save_watchdog_settings();
            }
        });
        let this_for_wd5 = self.downgrade();
        watchdog_target_row.connect_changed(move |_| {
            if let Some(this) = this_for_wd5.upgrade() {
                this.save_watchdog_settings();
            }
        });

        content.append(&watchdog_group);

        // Security group
        let security_group = adw::PreferencesGroup::builder()
            .title("Security")
            .description("Script execution and profile security settings")
            .build();

        let sandbox_model = gtk::StringList::new(&["No Sandboxing", "Bubblewrap", "Firejail"]);
        let sandbox_row = adw::ComboRow::builder()
            .title("Script Sandboxing")
            .subtitle("Method for isolating script execution")
            .model(&sandbox_model)
            .build();
        security_group.add(&sandbox_row);
        *imp.sandbox_row.borrow_mut() = Some(sandbox_row.clone());

        let encryption_row = adw::SwitchRow::builder()
            .title("Profile Encryption")
            .subtitle("Encrypt stored profile credentials")
            .active(false)
            .build();
        security_group.add(&encryption_row);
        *imp.encryption_row.borrow_mut() = Some(encryption_row.clone());

        let encryption_key_row = adw::PasswordEntryRow::builder()
            .title("Encryption Key")
            .build();
        security_group.add(&encryption_key_row);
        *imp.encryption_key_row.borrow_mut() = Some(encryption_key_row.clone());

        // Connect security save handlers
        let this_for_sec = self.downgrade();
        sandbox_row.connect_selected_notify(move |_| {
            if let Some(this) = this_for_sec.upgrade() {
                this.save_security_settings();
            }
        });
        let this_for_sec2 = self.downgrade();
        encryption_row.connect_active_notify(move |_| {
            if let Some(this) = this_for_sec2.upgrade() {
                this.save_security_settings();
            }
        });
        let this_for_sec3 = self.downgrade();
        encryption_key_row.connect_changed(move |_| {
            if let Some(this) = this_for_sec3.upgrade() {
                this.save_security_settings();
            }
        });

        content.append(&security_group);

        // Hotkeys group
        let hotkeys_group = adw::PreferencesGroup::builder()
            .title("Keyboard Shortcuts")
            .description("Quick access shortcuts for profile switching")
            .build();

        let hotkeys_enabled_row = adw::SwitchRow::builder()
            .title("Enable Profile Hotkeys")
            .subtitle("Use keyboard shortcuts to switch profiles")
            .active(false)
            .build();
        hotkeys_group.add(&hotkeys_enabled_row);
        *imp.hotkeys_enabled_row.borrow_mut() = Some(hotkeys_enabled_row.clone());

        // Connect hotkeys enabled save
        let this_for_hk = self.downgrade();
        hotkeys_enabled_row.connect_active_notify(move |row| {
            if let Some(this) = this_for_hk.upgrade() {
                this.save_hotkeys_enabled(row.is_active());
            }
        });

        let manage_hotkeys_row = adw::ActionRow::builder()
            .title("Configure Hotkeys")
            .subtitle("Assign keyboard shortcuts to profiles")
            .activatable(true)
            .build();
        let hotkey_icon = gtk::Image::from_icon_name("input-keyboard-symbolic");
        manage_hotkeys_row.add_suffix(&hotkey_icon);
        let hotkey_arrow = gtk::Image::from_icon_name("go-next-symbolic");
        manage_hotkeys_row.add_suffix(&hotkey_arrow);
        hotkeys_group.add(&manage_hotkeys_row);

        // Connect manage hotkeys
        let this_for_hotkeys = self.downgrade();
        manage_hotkeys_row.connect_activated(move |_| {
            if let Some(this) = this_for_hotkeys.upgrade() {
                this.show_manage_hotkeys_dialog();
            }
        });

        content.append(&hotkeys_group);

        // About group
        let about_group = adw::PreferencesGroup::builder()
            .title("About")
            .build();

        let version_row = adw::ActionRow::builder()
            .title("Version")
            .subtitle(env!("CARGO_PKG_VERSION"))
            .build();
        about_group.add(&version_row);

        let project_row = adw::ActionRow::builder()
            .title("Project Website")
            .subtitle("https://github.com/christosdaggas/network-manager")
            .activatable(true)
            .build();
        let link_icon = gtk::Image::from_icon_name("web-browser-symbolic");
        project_row.add_suffix(&link_icon);
        let project_arrow = gtk::Image::from_icon_name("go-next-symbolic");
        project_row.add_suffix(&project_arrow);
        about_group.add(&project_row);

        // Connect project website click
        project_row.connect_activated(|_| {
            if let Err(e) = gio::AppInfo::launch_default_for_uri("https://github.com/christosdaggas/network-manager", None::<&gio::AppLaunchContext>) {
                tracing::error!("Failed to open project website: {}", e);
            }
        });

        let report_issue_row = adw::ActionRow::builder()
            .title("Report an Issue")
            .subtitle("https://github.com/christosdaggas/network-manager/issues")
            .activatable(true)
            .build();
        let issue_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        report_issue_row.add_suffix(&issue_icon);
        let issue_arrow = gtk::Image::from_icon_name("go-next-symbolic");
        report_issue_row.add_suffix(&issue_arrow);
        about_group.add(&report_issue_row);

        // Connect report issue click
        report_issue_row.connect_activated(|_| {
            if let Err(e) = gio::AppInfo::launch_default_for_uri("https://github.com/christosdaggas/network-manager/issues", None::<&gio::AppLaunchContext>) {
                tracing::error!("Failed to open issues page: {}", e);
            }
        });

        content.append(&about_group);
    }

    /// Get the profiles directory path (display only - actual profiles are in cache)
    fn get_profiles_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_DIR_NAME)
    }

    /// Get the profiles cache file path
    fn get_profiles_cache_file() -> PathBuf {
        Self::get_profiles_dir().join("profiles_cache.json")
    }

    /// Get the logs file path (logs are stored in config directory as JSON)
    fn get_logs_dir() -> PathBuf {
        let base = glib::user_config_dir();
        base.join(crate::models::CONFIG_DIR_NAME)
    }

    /// Get profile names and IDs from the profiles cache.
    /// Returns (display_names, profile_ids) vectors.
    fn get_profile_names_and_ids() -> (Vec<String>, Vec<String>) {
        let cache_file = Self::get_profiles_cache_file();
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(profiles) = serde_json::from_str::<Vec<Profile>>(&content) {
                let names: Vec<String> = profiles.iter().map(|p| p.name().to_string()).collect();
                let ids: Vec<String> = profiles.iter().map(|p| p.id().to_string()).collect();
                if !names.is_empty() {
                    return (names, ids);
                }
            }
        }
        // Return default placeholder if no profiles found
        (vec!["(No profiles available)".to_string()], vec![String::new()])
    }

    /// Show file dialog for exporting profiles
    fn show_export_dialog(page: &Self, cache_file: &PathBuf) {
        let cache_file = cache_file.clone();
        let window = page.root().and_downcast::<gtk::Window>();
        
        let dialog = gtk::FileDialog::builder()
            .title("Export Profiles")
            .initial_name("network-manager-profiles.json")
            .build();
        
        // Use a flag to prevent double execution
        let executed = std::rc::Rc::new(std::cell::Cell::new(false));
        let executed_clone = executed.clone();
        
        dialog.save(
            window.as_ref(),
            None::<&gio::Cancellable>,
            move |result| {
                if executed_clone.get() {
                    return;
                }
                executed_clone.set(true);
                
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        Self::export_profiles(&cache_file, &path);
                    }
                }
            },
        );
    }

    /// Show file dialog for importing profiles
    fn show_import_dialog(page: &Self, cache_file: &PathBuf) {
        let cache_file = cache_file.clone();
        let gtk_window = page.root().and_downcast::<gtk::Window>();
        // Get MainWindow by downcasting from gtk::Window
        let main_window: Option<MainWindow> = gtk_window.as_ref()
            .and_then(|w| w.clone().downcast::<MainWindow>().ok());
        
        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.json");
        filter.set_name(Some("JSON files"));
        
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        
        let dialog = gtk::FileDialog::builder()
            .title("Import Profiles")
            .filters(&filters)
            .build();
        
        // Use a flag to prevent double execution
        let executed = std::rc::Rc::new(std::cell::Cell::new(false));
        let executed_clone = executed.clone();
        
        dialog.open(
            gtk_window.as_ref(),
            None::<&gio::Cancellable>,
            move |result| {
                if executed_clone.get() {
                    return;
                }
                executed_clone.set(true);
                
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        Self::import_profiles(&cache_file, &path);
                        // Reload profiles into the UI
                        if let Some(ref win) = main_window {
                            win.reload_profiles_from_cache();
                            win.show_toast("Profiles imported successfully");
                        }
                    }
                }
            },
        );
    }

    /// Export all profiles to a JSON file
    fn export_profiles(cache_file: &PathBuf, export_path: &PathBuf) {
        // Read profiles from cache file
        let profiles: Vec<Profile> = if cache_file.exists() {
            match std::fs::read_to_string(cache_file) {
                Ok(content) => {
                    match serde_json::from_str::<Vec<Profile>>(&content) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Failed to parse profiles cache: {}", e);
                            Vec::new()
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to read profiles cache: {}", e);
                    Vec::new()
                }
            }
        } else {
            tracing::warn!("No profiles cache file found at {:?}", cache_file);
            Vec::new()
        };
        
        // Write export file
        let export_data = serde_json::json!({
            "version": "1.0",
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "profiles": profiles
        });
        
        match serde_json::to_string_pretty(&export_data) {
            Ok(json) => {
                if let Err(e) = std::fs::write(export_path, json) {
                    tracing::error!("Failed to write export file: {}", e);
                } else {
                    tracing::info!("Exported {} profiles to {:?}", profiles.len(), export_path);
                }
            }
            Err(e) => tracing::error!("Failed to serialize profiles: {}", e),
        }
    }

    /// Import profiles from a JSON file
    fn import_profiles(cache_file: &PathBuf, import_path: &PathBuf) {
        // Ensure config directory exists
        if let Some(parent) = cache_file.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create config directory: {}", e);
                return;
            }
        }
        
        // Read import file
        let content = match std::fs::read_to_string(import_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to read import file: {}", e);
                return;
            }
        };
        
        // Parse JSON
        let import_data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to parse import file: {}", e);
                return;
            }
        };
        
        // Extract profiles
        let imported_profiles: Vec<Profile> = match import_data.get("profiles") {
            Some(profiles_value) => {
                match serde_json::from_value::<Vec<Profile>>(profiles_value.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Failed to parse profiles from import file: {}", e);
                        return;
                    }
                }
            }
            None => {
                tracing::error!("Invalid import file: missing 'profiles' array");
                return;
            }
        };
        
        // Load existing profiles from cache
        let mut existing_profiles: Vec<Profile> = if cache_file.exists() {
            match std::fs::read_to_string(cache_file) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        
        // Merge profiles (imported profiles override existing by ID)
        let mut profiles_map: std::collections::HashMap<String, Profile> = 
            existing_profiles.drain(..).map(|p| (p.id().to_string(), p)).collect();
        
        let imported_count = imported_profiles.len();
        for profile in imported_profiles {
            profiles_map.insert(profile.id().to_string(), profile);
        }
        
        let merged_profiles: Vec<Profile> = profiles_map.into_values().collect();
        
        // Write back to cache file
        match serde_json::to_string_pretty(&merged_profiles) {
            Ok(json) => {
                if let Err(e) = std::fs::write(cache_file, json) {
                    tracing::error!("Failed to write profiles cache: {}", e);
                } else {
                    tracing::info!("Imported {} profiles from {:?}", imported_count, import_path);
                    tracing::info!("Total profiles in cache: {}", merged_profiles.len());
                }
            }
            Err(e) => tracing::error!("Failed to serialize profiles: {}", e),
        }
    }

    /// Get the currently selected theme.
    pub fn selected_theme(&self) -> ThemePreference {
        let imp = self.imp();
        if let Some(theme_row) = imp.theme_row.borrow().as_ref() {
            match theme_row.selected() {
                0 => ThemePreference::System,
                1 => ThemePreference::Light,
                2 => ThemePreference::Dark,
                _ => ThemePreference::System,
            }
        } else {
            ThemePreference::System
        }
    }

    /// Set the theme selection.
    pub fn set_theme(&self, theme: ThemePreference) {
        let imp = self.imp();
        if let Some(theme_row) = imp.theme_row.borrow().as_ref() {
            let position = match theme {
                ThemePreference::System => 0,
                ThemePreference::Light => 1,
                ThemePreference::Dark => 2,
            };
            theme_row.set_selected(position);
        }
    }

    /// Check if auto-switch is enabled.
    pub fn auto_switch_enabled(&self) -> bool {
        let imp = self.imp();
        imp.auto_switch_row.borrow().as_ref().map_or(false, |r| r.is_active())
    }

    /// Get the auto-switch interval in seconds.
    pub fn auto_switch_interval(&self) -> u32 {
        let imp = self.imp();
        imp.auto_switch_interval.borrow().as_ref().map_or(30, |r| r.value() as u32)
    }

    /// Check if notifications are enabled.
    pub fn notifications_enabled(&self) -> bool {
        let imp = self.imp();
        imp.notifications_row.borrow().as_ref().map_or(true, |r| r.is_active())
    }

    /// Show the manage schedules dialog.
    fn show_manage_schedules_dialog(&self) {
        let Some(root) = self.root() else { return };
        let Some(window) = root.downcast_ref::<gtk::Window>() else { return };

        let dialog = adw::Dialog::builder()
            .title("Manage Schedules")
            .content_width(500)
            .content_height(400)
            .build();

        // Header bar with close and add buttons
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
        add_btn.set_tooltip_text(Some("Add Schedule"));
        add_btn.add_css_class("flat");
        header.pack_start(&add_btn);

        // Main content
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);

        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let schedules_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Status page for empty state
        let empty_page = adw::StatusPage::builder()
            .icon_name("alarm-symbolic")
            .title("No Schedules")
            .description("Add a schedule to automatically activate profiles at specific times.\n\nClick the + button to create your first schedule.")
            .build();
        schedules_box.append(&empty_page);

        scrolled.set_child(Some(&schedules_box));
        content.append(&scrolled);

        // Help text
        let help_label = gtk::Label::builder()
            .label("Schedules use cron format: minute hour day-of-month month day-of-week\nExample: \"30 9 * * 1-5\" = 9:30 AM on weekdays")
            .wrap(true)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        help_label.add_css_class("dim-label");
        help_label.add_css_class("caption");
        content.append(&help_label);

        dialog.set_child(Some(&content));

        // Connect add button
        let dialog_weak = glib::WeakRef::new();
        dialog_weak.set(Some(&dialog));
        add_btn.connect_clicked(move |_| {
            if let Some(dlg) = dialog_weak.upgrade() {
                Self::show_add_schedule_dialog(&dlg);
            }
        });

        dialog.present(Some(window));
    }

    /// Show the add schedule dialog with form fields.
    fn show_add_schedule_dialog(parent: &adw::Dialog) {
        let dialog = adw::Dialog::builder()
            .title("Add Schedule")
            .content_width(400)
            .content_height(450)
            .build();

        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let save_btn = gtk::Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        header.pack_end(&save_btn);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);

        let prefs_page = adw::PreferencesPage::new();

        // Schedule info group
        let info_group = adw::PreferencesGroup::builder()
            .title("Schedule Information")
            .build();

        let name_row = adw::EntryRow::builder()
            .title("Description")
            .build();
        name_row.set_text("Daily Work Profile");
        info_group.add(&name_row);

        // Profile selection - load from application's profiles
        let (profile_names, profile_ids) = Self::get_profile_names_and_ids();
        let profile_model = gtk::StringList::new(&profile_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let profile_row = adw::ComboRow::builder()
            .title("Profile")
            .subtitle("Profile to activate on schedule")
            .model(&profile_model)
            .build();
        info_group.add(&profile_row);

        prefs_page.add(&info_group);

        // Time group
        let time_group = adw::PreferencesGroup::builder()
            .title("Activation Time")
            .build();

        let hour_row = adw::SpinRow::builder()
            .title("Hour (0-23)")
            .adjustment(&gtk::Adjustment::new(9.0, 0.0, 23.0, 1.0, 1.0, 0.0))
            .build();
        time_group.add(&hour_row);

        let minute_row = adw::SpinRow::builder()
            .title("Minute (0-59)")
            .adjustment(&gtk::Adjustment::new(0.0, 0.0, 59.0, 1.0, 5.0, 0.0))
            .build();
        time_group.add(&minute_row);

        prefs_page.add(&time_group);

        // Days group
        let days_group = adw::PreferencesGroup::builder()
            .title("Days of Week")
            .description("Select days when the schedule should run")
            .build();

        let mon_row = adw::SwitchRow::builder().title("Monday").active(true).build();
        let tue_row = adw::SwitchRow::builder().title("Tuesday").active(true).build();
        let wed_row = adw::SwitchRow::builder().title("Wednesday").active(true).build();
        let thu_row = adw::SwitchRow::builder().title("Thursday").active(true).build();
        let fri_row = adw::SwitchRow::builder().title("Friday").active(true).build();
        let sat_row = adw::SwitchRow::builder().title("Saturday").active(false).build();
        let sun_row = adw::SwitchRow::builder().title("Sunday").active(false).build();

        days_group.add(&mon_row);
        days_group.add(&tue_row);
        days_group.add(&wed_row);
        days_group.add(&thu_row);
        days_group.add(&fri_row);
        days_group.add(&sat_row);
        days_group.add(&sun_row);

        prefs_page.add(&days_group);

        // Options group
        let options_group = adw::PreferencesGroup::builder()
            .title("Options")
            .build();

        let enabled_row = adw::SwitchRow::builder()
            .title("Enabled")
            .subtitle("Schedule will trigger when enabled")
            .active(true)
            .build();
        options_group.add(&enabled_row);

        let one_shot_row = adw::SwitchRow::builder()
            .title("One-shot")
            .subtitle("Run once then disable")
            .active(false)
            .build();
        options_group.add(&one_shot_row);

        prefs_page.add(&options_group);

        content.append(&prefs_page);
        dialog.set_child(Some(&content));

        // Connect save button
        let dialog_weak = glib::WeakRef::new();
        dialog_weak.set(Some(&dialog));
        let parent_weak = glib::WeakRef::new();
        parent_weak.set(Some(parent));
        // Clone profile_ids for the closure
        let profile_ids_clone = profile_ids.clone();
        let profile_row_weak = profile_row.downgrade();
        save_btn.connect_clicked(move |_| {
            let hour = hour_row.value() as u32;
            let minute = minute_row.value() as u32;
            let description = name_row.text().to_string();
            let is_enabled = enabled_row.is_active();
            let is_one_shot = one_shot_row.is_active();
            
            // Get selected profile ID
            let selected_profile_id = profile_row_weak.upgrade()
                .map(|row| row.selected() as usize)
                .and_then(|idx| profile_ids_clone.get(idx).cloned())
                .unwrap_or_default();
            
            // Build days string
            let mut days = Vec::new();
            if mon_row.is_active() { days.push("1"); }
            if tue_row.is_active() { days.push("2"); }
            if wed_row.is_active() { days.push("3"); }
            if thu_row.is_active() { days.push("4"); }
            if fri_row.is_active() { days.push("5"); }
            if sat_row.is_active() { days.push("6"); }
            if sun_row.is_active() { days.push("0"); }
            
            let days_str = if days.len() == 7 { "*".to_string() } else { days.join(",") };
            let cron = format!("{} {} * * {}", minute, hour, days_str);
            
            // Create schedule entry with actual profile ID
            let schedule = ScheduleEntry {
                id: uuid::Uuid::new_v4().to_string(),
                profile_id: selected_profile_id,
                cron_expression: cron.clone(),
                enabled: is_enabled,
                one_shot: is_one_shot,
                description: if description.is_empty() { None } else { Some(description.clone()) },
            };
            
            // Save to config via parent dialog -> window -> app
            if let Some(parent_dlg) = parent_weak.upgrade() {
                if let Some(root) = parent_dlg.root() {
                    if let Some(window) = root.downcast_ref::<gtk::Window>() {
                        if let Some(app) = window.application() {
                            if let Some(network_app) = app.downcast_ref::<crate::application::Application>() {
                                let mut config = network_app.config();
                                config.schedules.push(schedule);
                                network_app.update_config(config);
                                info!("Schedule saved: {}", cron);
                            }
                        }
                    }
                }
            }
            
            // Close dialog
            if let Some(dlg) = dialog_weak.upgrade() {
                dlg.close();
            }
        });

        dialog.present(Some(parent));
    }

    /// Show the manage hotkeys dialog.
    fn show_manage_hotkeys_dialog(&self) {
        let Some(root) = self.root() else { return };
        let Some(window) = root.downcast_ref::<gtk::Window>() else { return };

        let dialog = adw::Dialog::builder()
            .title("Configure Hotkeys")
            .content_width(500)
            .content_height(450)
            .build();

        // Header bar
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
        add_btn.set_tooltip_text(Some("Add Hotkey"));
        add_btn.add_css_class("flat");
        header.pack_start(&add_btn);

        // Main content
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);

        let prefs_page = adw::PreferencesPage::new();

        // Info group
        let info_group = adw::PreferencesGroup::builder()
            .title("Profile Hotkeys")
            .description("Global keyboard shortcuts for quick profile switching")
            .build();

        // Example hotkey entries (would be populated from config)
        let hotkey1 = adw::ActionRow::builder()
            .title("Work Profile")
            .subtitle("Ctrl+Alt+1")
            .build();
        let edit_btn1 = gtk::Button::from_icon_name("document-edit-symbolic");
        edit_btn1.set_valign(gtk::Align::Center);
        edit_btn1.add_css_class("flat");
        hotkey1.add_suffix(&edit_btn1);
        let del_btn1 = gtk::Button::from_icon_name("user-trash-symbolic");
        del_btn1.set_valign(gtk::Align::Center);
        del_btn1.add_css_class("flat");
        hotkey1.add_suffix(&del_btn1);
        info_group.add(&hotkey1);

        let hotkey2 = adw::ActionRow::builder()
            .title("Home Profile")
            .subtitle("Ctrl+Alt+2")
            .build();
        let edit_btn2 = gtk::Button::from_icon_name("document-edit-symbolic");
        edit_btn2.set_valign(gtk::Align::Center);
        edit_btn2.add_css_class("flat");
        hotkey2.add_suffix(&edit_btn2);
        let del_btn2 = gtk::Button::from_icon_name("user-trash-symbolic");
        del_btn2.set_valign(gtk::Align::Center);
        del_btn2.add_css_class("flat");
        hotkey2.add_suffix(&del_btn2);
        info_group.add(&hotkey2);

        prefs_page.add(&info_group);

        // Help group
        let help_group = adw::PreferencesGroup::builder()
            .title("How to Use")
            .build();

        let help_row = adw::ActionRow::builder()
            .title("About Hotkeys")
            .subtitle("Hotkeys work globally when the app is running. Press the shortcut to instantly switch to the assigned profile.")
            .build();
        help_group.add(&help_row);

        let note_row = adw::ActionRow::builder()
            .title("Note")
            .subtitle("Some shortcuts may conflict with system or other application shortcuts.")
            .build();
        let warning_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
        note_row.add_prefix(&warning_icon);
        help_group.add(&note_row);

        prefs_page.add(&help_group);

        content.append(&prefs_page);
        dialog.set_child(Some(&content));

        // Connect add button
        let dialog_weak = glib::WeakRef::new();
        dialog_weak.set(Some(&dialog));
        add_btn.connect_clicked(move |_| {
            if let Some(dlg) = dialog_weak.upgrade() {
                Self::show_add_hotkey_dialog(&dlg);
            }
        });

        dialog.present(Some(window));
    }

    /// Show dialog to add a new hotkey.
    fn show_add_hotkey_dialog(parent: &adw::Dialog) {
        let dialog = adw::Dialog::builder()
            .title("Add Hotkey")
            .content_width(350)
            .content_height(300)
            .build();

        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let save_btn = gtk::Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        header.pack_end(&save_btn);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);

        let prefs_page = adw::PreferencesPage::new();

        let group = adw::PreferencesGroup::builder()
            .title("Hotkey Configuration")
            .build();

        // Profile selection - load from actual profiles
        let (profile_names, profile_ids) = Self::get_profile_names_and_ids();
        let profile_model = gtk::StringList::new(&profile_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let profile_row = adw::ComboRow::builder()
            .title("Profile")
            .subtitle("Profile to activate with this hotkey")
            .model(&profile_model)
            .build();
        group.add(&profile_row);

        // Modifier keys
        let ctrl_row = adw::SwitchRow::builder()
            .title("Ctrl")
            .active(true)
            .build();
        group.add(&ctrl_row);

        let alt_row = adw::SwitchRow::builder()
            .title("Alt")
            .active(true)
            .build();
        group.add(&alt_row);

        let shift_row = adw::SwitchRow::builder()
            .title("Shift")
            .active(false)
            .build();
        group.add(&shift_row);

        let super_row = adw::SwitchRow::builder()
            .title("Super")
            .active(false)
            .build();
        group.add(&super_row);

        // Key entry
        let key_row = adw::EntryRow::builder()
            .title("Key")
            .build();
        key_row.set_text("1");
        group.add(&key_row);

        prefs_page.add(&group);
        content.append(&prefs_page);

        dialog.set_child(Some(&content));

        // Connect save
        let dialog_weak = glib::WeakRef::new();
        dialog_weak.set(Some(&dialog));
        let parent_weak = glib::WeakRef::new();
        parent_weak.set(Some(parent));
        let profile_names_clone = profile_names.clone();
        let profile_ids_clone = profile_ids.clone();
        save_btn.connect_clicked(move |_| {
            let mut modifiers = Vec::new();
            if ctrl_row.is_active() { modifiers.push("Ctrl".to_string()); }
            if alt_row.is_active() { modifiers.push("Alt".to_string()); }
            if shift_row.is_active() { modifiers.push("Shift".to_string()); }
            if super_row.is_active() { modifiers.push("Super".to_string()); }
            
            let key = key_row.text().to_string();
            let profile_idx = profile_row.selected() as usize;
            
            // Get profile name and ID from loaded profiles
            let profile_name = profile_names_clone.get(profile_idx)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            let profile_id = profile_ids_clone.get(profile_idx)
                .cloned()
                .unwrap_or_default();
            
            // Create hotkey entry with actual profile ID
            let hotkey = HotkeyEntry {
                id: uuid::Uuid::new_v4().to_string(),
                profile_id,
                profile_name: profile_name.clone(),
                modifiers,
                key: key.clone(),
                enabled: true,
            };
            
            let shortcut = hotkey.shortcut_string();
            
            // Save to config via parent dialog -> window -> app
            if let Some(parent_dlg) = parent_weak.upgrade() {
                if let Some(root) = parent_dlg.root() {
                    if let Some(window) = root.downcast_ref::<gtk::Window>() {
                        if let Some(app) = window.application() {
                            if let Some(network_app) = app.downcast_ref::<crate::application::Application>() {
                                let mut config = network_app.config();
                                config.hotkeys.push(hotkey);
                                network_app.update_config(config);
                                info!("Hotkey saved: {} -> {}", shortcut, profile_name);
                            }
                        }
                    }
                }
            }
            
            if let Some(dlg) = dialog_weak.upgrade() {
                dlg.close();
            }
        });

        dialog.present(Some(parent));
    }

    /// Show the auto-switch rules management dialog.
    fn show_manage_rules_dialog(&self) {
        let Some(root) = self.root() else { return };
        let Some(window) = root.downcast_ref::<gtk::Window>() else { return };

        let dialog = adw::Dialog::builder()
            .title("Auto-Switch Rules")
            .content_width(550)
            .content_height(450)
            .build();

        // Header bar with add button
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .build();

        let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
        add_btn.set_tooltip_text(Some("Add Rule"));
        add_btn.add_css_class("flat");
        header.pack_start(&add_btn);

        // Main content
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);

        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let rules_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Info about rule types
        let rules_group = adw::PreferencesGroup::builder()
            .title("Available Rule Types")
            .description("Rules are configured per-profile in the profile editor")
            .build();

        let ssid_row = adw::ActionRow::builder()
            .title("Wi-Fi SSID")
            .subtitle("Activate when connected to a specific network")
            .build();
        let ssid_icon = gtk::Image::from_icon_name("network-wireless-symbolic");
        ssid_row.add_prefix(&ssid_icon);
        rules_group.add(&ssid_row);

        let gateway_row = adw::ActionRow::builder()
            .title("Gateway MAC")
            .subtitle("Match by router MAC address")
            .build();
        let gw_icon = gtk::Image::from_icon_name("network-wired-symbolic");
        gateway_row.add_prefix(&gw_icon);
        rules_group.add(&gateway_row);

        let ping_row = adw::ActionRow::builder()
            .title("Ping Target")
            .subtitle("Activate when a host is reachable")
            .build();
        let ping_icon = gtk::Image::from_icon_name("network-server-symbolic");
        ping_row.add_prefix(&ping_icon);
        rules_group.add(&ping_row);

        let time_row = adw::ActionRow::builder()
            .title("Time Window")
            .subtitle("Activate during specific hours/days")
            .build();
        let time_icon = gtk::Image::from_icon_name("appointment-soon-symbolic");
        time_row.add_prefix(&time_icon);
        rules_group.add(&time_row);

        let interface_row = adw::ActionRow::builder()
            .title("Interface State")
            .subtitle("Match network interface up/down state")
            .build();
        let if_icon = gtk::Image::from_icon_name("preferences-system-network-symbolic");
        interface_row.add_prefix(&if_icon);
        rules_group.add(&interface_row);

        rules_box.append(&rules_group);

        // Help text
        let help_box = adw::Clamp::builder()
            .maximum_size(500)
            .build();
        let help_label = gtk::Label::builder()
            .label("To add rules to a profile, edit the profile and configure auto-switch conditions.\nRules can be combined with AND/OR logic for complex matching.")
            .wrap(true)
            .margin_top(16)
            .build();
        help_label.add_css_class("dim-label");
        help_box.set_child(Some(&help_label));
        rules_box.append(&help_box);

        scrolled.set_child(Some(&rules_box));
        content.append(&scrolled);

        dialog.set_child(Some(&content));

        // Connect add button
        let dialog_weak = glib::WeakRef::new();
        dialog_weak.set(Some(&dialog));
        add_btn.connect_clicked(move |_| {
            if let Some(dlg) = dialog_weak.upgrade() {
                let info_dialog = adw::AlertDialog::builder()
                    .heading("Add Auto-Switch Rule")
                    .body("Auto-switch rules are configured per-profile.\n\nOpen a profile for editing and add rules in the Auto-Switch section.")
                    .build();
                info_dialog.add_response("ok", "Got it");
                info_dialog.set_default_response(Some("ok"));
                info_dialog.present(Some(&dlg));
            }
        });

        dialog.present(Some(window));
    }

    /// Get the Application and update config.
    fn update_app_config<F: FnOnce(&mut crate::models::AppConfig)>(&self, f: F) {
        if let Some(root) = self.root() {
            if let Some(window) = root.downcast_ref::<gtk::Window>() {
                if let Some(app) = window.application() {
                    if let Some(network_app) = app.downcast_ref::<crate::application::Application>() {
                        let mut config = network_app.config();
                        f(&mut config);
                        network_app.update_config(config);
                        info!("Settings saved to config");
                    }
                }
            }
        }
    }

    /// Save scheduling enabled setting.
    fn save_scheduling_enabled(&self, enabled: bool) {
        self.update_app_config(|config| {
            config.scheduling_enabled = enabled;
        });
    }

    /// Save watchdog settings.
    fn save_watchdog_settings(&self) {
        let imp = self.imp();
        
        let enabled = imp.watchdog_enabled_row.borrow().as_ref()
            .map(|r| r.is_active()).unwrap_or(false);
        let interval = imp.watchdog_interval_row.borrow().as_ref()
            .map(|r| r.value() as u32).unwrap_or(30);
        let target = imp.watchdog_target_row.borrow().as_ref()
            .map(|r| r.text().to_string()).unwrap_or_else(|| "8.8.8.8".to_string());
        let threshold = imp.watchdog_threshold_row.borrow().as_ref()
            .map(|r| r.value() as u32).unwrap_or(3);
        let action_idx = imp.watchdog_action_row.borrow().as_ref()
            .map(|r| r.selected()).unwrap_or(0);
        
        let action = match action_idx {
            0 => WatchdogAction::Notify,
            1 => WatchdogAction::Reconnect,
            2 => WatchdogAction::SwitchProfile,
            3 => WatchdogAction::RestartNetworkManager,
            _ => WatchdogAction::Notify,
        };
        
        self.update_app_config(|config| {
            config.watchdog = WatchdogConfig {
                enabled,
                check_interval_secs: interval,
                ping_target: target,
                failure_threshold: threshold,
                failure_action: action,
                fallback_profile_id: config.watchdog.fallback_profile_id.clone(),
            };
        });
    }

    /// Save security settings.
    fn save_security_settings(&self) {
        let imp = self.imp();
        
        let sandbox_idx = imp.sandbox_row.borrow().as_ref()
            .map(|r| r.selected()).unwrap_or(0);
        let encrypt = imp.encryption_row.borrow().as_ref()
            .map(|r| r.is_active()).unwrap_or(false);
        let key = imp.encryption_key_row.borrow().as_ref()
            .map(|r| r.text().to_string()).unwrap_or_default();
        
        let sandbox_mode = match sandbox_idx {
            0 => SandboxMode::None,
            1 => SandboxMode::Bubblewrap,
            2 => SandboxMode::Firejail,
            _ => SandboxMode::None,
        };
        
        self.update_app_config(|config| {
            config.sandbox_mode = sandbox_mode;
            config.encrypt_profiles = encrypt;
            config.encryption_key = if key.is_empty() { None } else { Some(key) };
        });
    }

    /// Save hotkeys enabled setting.
    fn save_hotkeys_enabled(&self, enabled: bool) {
        self.update_app_config(|config| {
            config.hotkeys_enabled = enabled;
        });
    }

    /// Load config values into UI widgets.
    pub fn load_from_config(&self, config: &crate::models::AppConfig) {
        let imp = self.imp();
        
        // Theme
        if let Some(row) = imp.theme_row.borrow().as_ref() {
            let idx = match config.theme {
                ThemePreference::System => 0,
                ThemePreference::Light => 1,
                ThemePreference::Dark => 2,
            };
            row.set_selected(idx);
        }
        
        // Auto-switch
        if let Some(row) = imp.auto_switch_row.borrow().as_ref() {
            row.set_active(config.auto_switch_enabled);
        }
        if let Some(row) = imp.auto_switch_interval.borrow().as_ref() {
            row.set_value(config.auto_switch_interval_secs as f64);
        }
        
        // Notifications
        if let Some(row) = imp.notifications_row.borrow().as_ref() {
            row.set_active(config.show_notifications);
        }
        
        // Start minimized
        if let Some(row) = imp.start_minimized_row.borrow().as_ref() {
            row.set_active(config.start_minimized);
        }
        
        // Scheduling
        if let Some(row) = imp.scheduling_enabled_row.borrow().as_ref() {
            row.set_active(config.scheduling_enabled);
        }
        
        // Watchdog
        if let Some(row) = imp.watchdog_enabled_row.borrow().as_ref() {
            row.set_active(config.watchdog.enabled);
        }
        if let Some(row) = imp.watchdog_interval_row.borrow().as_ref() {
            row.set_value(config.watchdog.check_interval_secs as f64);
        }
        if let Some(row) = imp.watchdog_target_row.borrow().as_ref() {
            row.set_text(&config.watchdog.ping_target);
        }
        if let Some(row) = imp.watchdog_threshold_row.borrow().as_ref() {
            row.set_value(config.watchdog.failure_threshold as f64);
        }
        if let Some(row) = imp.watchdog_action_row.borrow().as_ref() {
            let idx = match config.watchdog.failure_action {
                WatchdogAction::Notify => 0,
                WatchdogAction::Reconnect => 1,
                WatchdogAction::SwitchProfile => 2,
                WatchdogAction::RestartNetworkManager => 3,
            };
            row.set_selected(idx);
        }
        
        // Security
        if let Some(row) = imp.sandbox_row.borrow().as_ref() {
            let idx = match config.sandbox_mode {
                SandboxMode::None => 0,
                SandboxMode::Bubblewrap => 1,
                SandboxMode::Firejail => 2,
            };
            row.set_selected(idx);
        }
        if let Some(row) = imp.encryption_row.borrow().as_ref() {
            row.set_active(config.encrypt_profiles);
        }
        if let Some(row) = imp.encryption_key_row.borrow().as_ref() {
            if let Some(ref key) = config.encryption_key {
                row.set_text(key);
            }
        }
        
        // Hotkeys
        if let Some(row) = imp.hotkeys_enabled_row.borrow().as_ref() {
            row.set_active(config.hotkeys_enabled);
        }
        
        info!("Loaded settings from config");
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}
