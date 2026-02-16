//! Adwaita application root object and lifecycle wiring.

use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{info, warn};

use crate::models::AppConfig;
use crate::storage::DataStore;
use crate::ui::MainWindow;
use crate::{APP_ID, APP_NAME, VERSION};

/// Global Tokio runtime for async operations.
#[allow(dead_code)]
static TOKIO_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

/// Get the global Tokio runtime handle.
#[allow(dead_code)]
pub fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

mod imp {
    use super::*;
    use crate::tray::TrayHandle;

    #[derive(Default)]
    pub struct Application {
        pub data_store: RefCell<Option<Arc<DataStore>>>,
        pub config: RefCell<AppConfig>,
        pub tray_handle: RefCell<Option<TrayHandle>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "CdNetworkManagerApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_actions();
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
            obj.set_accels_for_action("win.show-help-overlay", &["<primary>question"]);
        }
    }

    impl ApplicationImpl for Application {
        fn activate(&self) {
            let application = self.obj();

            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = MainWindow::new(&*application);

                if let Some(store) = self.data_store.borrow().clone() {
                    window.init_with_store(store);
                }

                window.upcast()
            };

            window.present();
        }

        fn startup(&self) {
            self.parent_startup();
            let obj = self.obj();

            info!("{} {} starting up", APP_NAME, VERSION);

            if let Some(display) = gtk::gdk::Display::default() {
                let icon_theme = gtk::IconTheme::for_display(&display);
                
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(exe_dir) = exe_path.parent() {
                        let dev_icons = exe_dir.join("../../data/icons");
                        if dev_icons.exists() {
                            if let Some(path_str) = dev_icons.canonicalize().ok().and_then(|p| p.to_str().map(String::from)) {
                                icon_theme.add_search_path(&path_str);
                            }
                        }
                    }
                }
                
                icon_theme.add_search_path("data/icons");
            }
            
            gtk::Window::set_default_icon_name(APP_ID);

            let data_store = Arc::new(DataStore::new());
            *self.data_store.borrow_mut() = Some(data_store);

            let config = DataStore::load_config().unwrap_or_default();
            *self.config.borrow_mut() = config.clone();

            obj.apply_theme(config.theme);
            
            if config.show_tray_icon {
                obj.start_tray();
            }

            obj.load_css();
            
            obj.start_background_services(&config);
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::FLAGS_NONE)
            .property("resource-base-path", "/com/chrisdaggas/network-manager")
            .build()
    }

    fn setup_actions(&self) {
        let action_quit = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| {
                app.quit();
            })
            .build();

        let action_about = gio::ActionEntry::builder("about")
            .activate(move |app: &Self, _, _| {
                app.show_about();
            })
            .build();

        let action_preferences = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Self, _, _| {
                app.show_preferences();
            })
            .build();

        self.add_action_entries([action_quit, action_about, action_preferences]);
    }

    fn load_css(&self) {
        let Some(display) = gtk::gdk::Display::default() else {
            warn!("No default display available; skipping CSS provider installation");
            return;
        };

        let provider = gtk::CssProvider::new();
        let css = include_str!("../data/style.css");
        provider.load_from_string(css);

        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    /// Apply the theme preference.
    pub fn apply_theme(&self, theme: crate::models::ThemePreference) {
        let style_manager = adw::StyleManager::default();
        match theme {
            crate::models::ThemePreference::System => {
                style_manager.set_color_scheme(adw::ColorScheme::Default);
            }
            crate::models::ThemePreference::Light => {
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
            }
            crate::models::ThemePreference::Dark => {
                style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
            }
        }
    }

    fn show_about(&self) {
        let window = self.active_window();

        let about = adw::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon(APP_ID)
            .developer_name("Christos A. Daggas")
            .version(VERSION)
            .copyright("© 2026 Christos A. Daggas")
            .license_type(gtk::License::MitX11)
            .website("https://chrisdaggas.com")
            .issue_url("https://github.com/christosdaggas/network-manager/issues")
            .developers(vec!["Christos A. Daggas"])
            .comments("Network and system profile manager for Linux")
            .release_notes("<p>Version 1.4.0 - February 2026</p><ul><li>Argon2id key derivation for profile encryption (replaces raw SHA-256)</li><li>Random per-message salt added to every encrypted payload</li><li>Encryption passphrase is no longer persisted to disk</li><li>Config, cache, and log files set strict 0600/0700 permissions</li><li>Key material zeroed from memory on drop (zeroize)</li><li>Cryptographic nonces generated with OsRng</li><li>Sandbox mode fails loudly when bwrap/firejail is missing</li><li>Removed shell-injection-prone execute_command(sh -c) API</li><li>Ping and DNS input validated against injection</li><li>Proxy settings check for GNOME schema before applying</li><li>Regex DoS protection with size-limited compilation</li><li>Compiled regexes cached across auto-switch cycles</li><li>Watchdog and auto-switch blocking calls offloaded from UI thread</li><li>Update checker response body size-limited to 512 KiB</li><li>Tokio runtime features narrowed to reduce binary size</li><li>DEB/RPM packages now declare NetworkManager dependency</li><li>Package architecture auto-detected (aarch64 support)</li><li>Bubblewrap uses existence check for /lib64 instead of hard bind</li></ul><p>Version 1.2.0 - February 2026</p><ul><li>Schedule Profile Activation - Schedule entries now use real profiles from your configuration</li><li>Hotkey Profile Binding - Hotkeys can be bound to actual profiles for quick switching</li><li>Automatic Schedule Execution - Scheduler now properly activates profiles at scheduled times</li><li>VPN Disconnect Support - Disconnect from VPN connections when switching profiles</li><li>MTU Configuration - Set custom MTU values per interface</li><li>MAC Address Cloning - Configure cloned/spoofed MAC addresses</li><li>DNS Search Domains - Configure search domains for DNS resolution</li><li>Hostname Configuration - Set static and pretty hostnames via profiles</li><li>Timezone Configuration - Automatic timezone switching per profile</li><li>Default Printer Selection - Set default printer when activating profiles</li><li>Environment Variables - Configure environment variables per profile</li><li>Sandbox Script Execution - Run automation scripts in bubblewrap or flatpak sandbox</li><li>Profile Encryption - Encrypt sensitive profile data with AES-256-GCM</li></ul><p>Version 1.1.0 - February 2026</p><ul><li>Collapsible Sidebar - Navigation sidebar toggles between expanded and icon-only collapsed mode</li><li>Automatic Update Check - Checks GitHub for newer releases on startup</li><li>What's New Section - Release notes visible in the About dialog</li><li>Improved Card Styling - Consistent shadow and border on all dashboard cards</li><li>Center-Aligned Icons - Navigation icons are properly centered when sidebar is collapsed</li></ul><p>Version 1.0.0 - Initial Release</p><ul><li>Dashboard with network status overview</li><li>Network profile management (create, edit, delete, apply)</li><li>System log viewer</li><li>Theme selector (system, light, dark)</li><li>D-Bus integration for network configuration</li><li>Autostart support</li></ul>")
            .build();

        about.present(window.as_ref());
    }

    fn show_preferences(&self) {
        let window = self.active_window();
        
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title("Preferences");
        
        let appearance_page = adw::PreferencesPage::new();
        appearance_page.set_title("Appearance");
        appearance_page.set_icon_name(Some("preferences-desktop-appearance-symbolic"));
        
        let theme_group = adw::PreferencesGroup::new();
        theme_group.set_title("Theme");
        
        let theme_model = gtk::StringList::new(&["Follow System", "Light", "Dark"]);
        let theme_row = adw::ComboRow::builder()
            .title("Color Scheme")
            .subtitle("Choose the application's appearance")
            .model(&theme_model)
            .build();
        
        let current_theme = self.config().theme;
        match current_theme {
            crate::models::ThemePreference::System => theme_row.set_selected(0),
            crate::models::ThemePreference::Light => theme_row.set_selected(1),
            crate::models::ThemePreference::Dark => theme_row.set_selected(2),
        }
        
        let app_weak = self.downgrade();
        theme_row.connect_selected_notify(move |row| {
            if let Some(app) = app_weak.upgrade() {
                let theme = match row.selected() {
                    0 => crate::models::ThemePreference::System,
                    1 => crate::models::ThemePreference::Light,
                    2 => crate::models::ThemePreference::Dark,
                    _ => crate::models::ThemePreference::System,
                };
                app.apply_theme(theme);
                let mut config = app.config();
                config.theme = theme;
                app.update_config(config);
            }
        });
        
        theme_group.add(&theme_row);
        appearance_page.add(&theme_group);
        
        // === Behavior Page ===
        let behavior_page = adw::PreferencesPage::new();
        behavior_page.set_title("Behavior");
        behavior_page.set_icon_name(Some("preferences-system-symbolic"));
        
        let startup_group = adw::PreferencesGroup::new();
        startup_group.set_title("Startup");
        
        let config = self.config();
        
        // Check actual autostart state
        let autostart_enabled = crate::autostart::is_autostart_enabled();
        
        let autostart_row = adw::SwitchRow::builder()
            .title("Start on Login")
            .subtitle("Automatically start Network Manager when you log in")
            .active(autostart_enabled)
            .build();
        
        let app_weak = self.downgrade();
        autostart_row.connect_active_notify(move |row| {
            let enabled = row.is_active();
            if let Err(e) = crate::autostart::set_autostart(enabled) {
                tracing::error!("Failed to set autostart: {}", e);
            } else {
                if let Some(app) = app_weak.upgrade() {
                    let mut config = app.config();
                    config.autostart_on_login = enabled;
                    app.update_config(config);
                }
            }
        });
        
        startup_group.add(&autostart_row);
        
        let tray_row = adw::SwitchRow::builder()
            .title("Show System Tray Icon")
            .subtitle("Display an icon in the system tray")
            .active(config.show_tray_icon)
            .build();
        
        let app_weak = self.downgrade();
        tray_row.connect_active_notify(move |row| {
            let enabled = row.is_active();
            if let Some(app) = app_weak.upgrade() {
                let mut config = app.config();
                config.show_tray_icon = enabled;
                app.update_config(config);
                
                // Note: Dynamic tray icon toggle requires app restart
                // Full integration would need libappindicator or ksni crate
                if enabled {
                    tracing::info!("System tray icon enabled (requires restart to take effect)");
                    if let Some(window) = app.active_window() {
                        if let Some(main_window) = window.downcast_ref::<MainWindow>() {
                            main_window.show_toast("Tray icon will appear after restart");
                        }
                    }
                } else {
                    tracing::info!("System tray icon disabled");
                }
            }
        });
        
        startup_group.add(&tray_row);
        
        behavior_page.add(&startup_group);
        
        let profile_group = adw::PreferencesGroup::new();
        profile_group.set_title("Profiles");
        
        let confirm_switch_row = adw::SwitchRow::builder()
            .title("Confirm Profile Switch")
            .subtitle("Ask for confirmation before switching profiles")
            .active(true)
            .build();
        profile_group.add(&confirm_switch_row);
        
        behavior_page.add(&profile_group);
        
        // Add pages to dialog
        dialog.add(&appearance_page);
        dialog.add(&behavior_page);
        
        dialog.present(window.as_ref());
    }

    /// Get the data store.
    pub fn data_store(&self) -> Option<Arc<DataStore>> {
        self.imp().data_store.borrow().clone()
    }

    /// Get the current configuration.
    pub fn config(&self) -> AppConfig {
        self.imp().config.borrow().clone()
    }

    /// Update and save configuration.
    pub fn update_config(&self, config: AppConfig) {
        *self.imp().config.borrow_mut() = config.clone();
        if let Some(store) = self.data_store() {
            let _ = store.save_config(&config);
        }
    }
    
    /// Start the system tray.
    fn start_tray(&self) {
        use crate::tray::{start_tray, TrayCommand};
        
        if let Some((handle, receiver)) = start_tray() {
            // Store the handle
            *self.imp().tray_handle.borrow_mut() = Some(handle);
            
            // Keep the app running when window is closed (tray mode)
            let _hold_guard = self.hold();
            
            // Process tray commands on the main thread
            let app_weak = self.downgrade();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let Some(app) = app_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                
                while let Ok(cmd) = receiver.try_recv() {
                    match cmd {
                        TrayCommand::ShowWindow => {
                            app.activate();
                        }
                        TrayCommand::ApplyProfile(profile_id) => {
                            if let Some(window) = app.active_window() {
                                if let Some(main_window) = window.downcast_ref::<MainWindow>() {
                                    let _ = gtk::prelude::WidgetExt::activate_action(main_window, "apply-profile", Some(&profile_id.to_variant()));
                                }
                            }
                        }
                        TrayCommand::Quit => {
                            // Quit the application
                            app.quit();
                        }
                    }
                }
                
                glib::ControlFlow::Continue
            });
            
            info!("System tray started");
        }
    }
    
    /// Update the tray with profile information.
    pub fn update_tray_profiles(&self, profiles: Vec<(String, String)>, active: Option<String>) {
        if let Some(handle) = self.imp().tray_handle.borrow().as_ref() {
            handle.set_profiles(profiles);
            handle.set_active_profile(active);
        }
    }

    /// Start background services (scheduler, watchdog, auto-switch).
    fn start_background_services(&self, config: &AppConfig) {
        // Start scheduler if scheduling is enabled and there are schedules
        if config.scheduling_enabled && !config.schedules.is_empty() {
            self.start_scheduler(config.schedules.clone());
        }

        // Start watchdog if enabled
        if config.watchdog.enabled {
            self.start_watchdog(config.watchdog.clone());
        }

        // Start auto-switch service
        self.start_autoswitch();
    }

    /// Start the profile scheduler.
    fn start_scheduler(&self, schedules: Vec<crate::models::ScheduleEntry>) {
        use crate::scheduler::SchedulerService;
        
        info!("Starting profile scheduler with {} schedule(s)", schedules.len());
        
        // Check every minute for schedule matches
        let app_weak = self.downgrade();
        let schedules_clone = schedules.clone();
        glib::timeout_add_seconds_local(60, move || {
            if let Some(app) = app_weak.upgrade() {
                let triggered = SchedulerService::check_schedules(&schedules_clone);
                for profile_id in triggered {
                    info!("Scheduler: activating profile {}", profile_id);
                    
                    // Apply the profile via MainWindow
                    if let Some(window) = app.active_window() {
                        if let Some(main_window) = window.downcast_ref::<MainWindow>() {
                            main_window.apply_profile(&profile_id);
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    /// Start the connection watchdog.
    ///
    /// The blocking `ping` call runs on a background thread so the GTK
    /// main loop is never stalled.
    fn start_watchdog(&self, config: crate::models::WatchdogConfig) {
        use crate::services::WatchdogService;
        
        let interval = config.check_interval_secs;
        info!("Starting connection watchdog (interval: {}s, target: {})", 
              interval, config.ping_target);
        
        let watchdog = std::sync::Arc::new(WatchdogService::new(config));
        let app_weak = self.downgrade();

        // Channel to receive notification signals from the background thread.
        let (tx, rx) = std::sync::mpsc::channel::<bool>();

        // Poll for notification results on the main thread.
        let app_weak_poll = app_weak.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            while let Ok(should_notify) = rx.try_recv() {
                if should_notify {
                    if let Some(app) = app_weak_poll.upgrade() {
                        if let Some(window) = app.active_window() {
                            if let Some(main_win) = window.downcast_ref::<MainWindow>() {
                                main_win.show_toast("Connection lost! Check your network.");
                            }
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        glib::timeout_add_seconds_local(interval, move || {
            let watchdog_inner = std::sync::Arc::clone(&watchdog);
            let tx_inner = tx.clone();

            // Run the blocking connectivity check off the main thread.
            std::thread::spawn(move || {
                let action = watchdog_inner.check();
                if let Some(action) = action {
                    info!("Watchdog triggered action: {:?}", action);

                    if let Err(e) = watchdog_inner.execute_action(action) {
                        tracing::error!("Watchdog action failed: {}", e);
                    }

                    if action == crate::models::WatchdogAction::Notify {
                        let _ = tx_inner.send(true);
                    }
                }
            });

            glib::ControlFlow::Continue
        });
    }

    /// Start the auto-switch service.
    ///
    /// Blocking network checks (`nmcli`, `ip`, `ping`) run on a
    /// background thread so the GTK main loop stays responsive.
    fn start_autoswitch(&self) {
        use crate::services::AutoSwitchService;
        use std::sync::{Arc, Mutex};
        
        info!("Starting auto-switch service (interval: 30s)");
        
        // Keep the service behind Arc<Mutex<>> so it can be shared with the
        // background thread while preserving its mutable regex cache.
        let service = Arc::new(Mutex::new(AutoSwitchService::new()));
        
        let app_weak = self.downgrade();

        // Channel to post matched profile IDs from background to main thread.
        let (tx, rx) = std::sync::mpsc::channel::<String>();

        // Poll for auto-switch results on the main thread.
        let app_weak_poll = app_weak.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            while let Ok(profile_id) = rx.try_recv() {
                if let Some(app) = app_weak_poll.upgrade() {
                    if let Some(window) = app.active_window() {
                        if let Some(main_window) = window.downcast_ref::<MainWindow>() {
                            let _ = gtk::prelude::WidgetExt::activate_action(
                                main_window,
                                "apply-profile",
                                Some(&profile_id.to_variant()),
                            );
                            main_window.show_toast("Auto-switched to profile");
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        // Check every 30 seconds for rule matches
        glib::timeout_add_seconds_local(30, move || {
            let Some(app) = app_weak.upgrade() else {
                return glib::ControlFlow::Continue;
            };

            // Snapshot the profiles on the main thread (cheap clone).
            let profiles = if let Some(store) = app.imp().data_store.borrow().as_ref() {
                store.profiles()
            } else {
                Vec::new()
            };

            let service_clone = Arc::clone(&service);
            let tx_inner = tx.clone();

            // Offload the blocking evaluation to a background thread.
            std::thread::spawn(move || {
                let matched_id = {
                    let mut svc = service_clone.lock().unwrap();
                    svc.evaluate_profiles(&profiles)
                };

                if let Some(profile_id) = matched_id {
                    info!("Auto-switch: Activating profile {}", profile_id);
                    let _ = tx_inner.send(profile_id);
                }
            });

            glib::ControlFlow::Continue
        });
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
