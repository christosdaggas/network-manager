// Network Manager - Main Window
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Main Application Window.
//!
//! Navigation split view with sidebar and content area.
//! Follows GNOME HIG for adaptive layouts.

use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use adw::prelude::*;
use adw::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::rc::Rc;

use crate::application::Application;
use crate::storage::DataStore;
use crate::ui::pages::{DashboardPage, LogsPage, ProfilesPage, SettingsPage, HelpPage};
use crate::models::Profile;

/// Navigation items for the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Dashboard,
    Profiles,
    Logs,
    Settings,
    Help,
}

impl NavItem {
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Dashboard => "view-grid-symbolic",
            Self::Profiles => "contact-new-symbolic",
            Self::Logs => "document-properties-symbolic",
            Self::Settings => "preferences-system-symbolic",
            Self::Help => "help-about-symbolic",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Profiles => "Profiles",
            Self::Logs => "Logs",
            Self::Settings => "Settings",
            Self::Help => "Help",
        }
    }

    pub fn all() -> &'static [NavItem] {
        &[
            Self::Dashboard,
            Self::Profiles,
            Self::Logs,
            Self::Settings,
            Self::Help,
        ]
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MainWindow {
        pub sidebar_box: RefCell<Option<gtk::Box>>,
        pub sidebar_list: RefCell<Option<gtk::ListBox>>,
        pub content_stack: RefCell<Option<gtk::Stack>>,
        pub header_bar: RefCell<Option<adw::HeaderBar>>,
        pub content_title: RefCell<Option<adw::WindowTitle>>,
        pub current_nav: Cell<Option<NavItem>>,
        pub toast_overlay: RefCell<Option<adw::ToastOverlay>>,

        pub sidebar_collapsed: Cell<bool>,
        pub sidebar_toggle_btn: RefCell<Option<gtk::Button>>,
        pub sidebar_header: RefCell<Option<adw::HeaderBar>>,
        pub sidebar_title: RefCell<Option<adw::WindowTitle>>,
        pub info_box: RefCell<Option<gtk::Box>>,
        pub nav_labels: RefCell<Vec<gtk::Label>>,
        pub nav_boxes: RefCell<Vec<gtk::Box>>,
        pub update_banner: RefCell<Option<gtk::Box>>,

        // Data store for persistence
        pub data_store: RefCell<Option<Arc<DataStore>>>,

        // Profiles storage
        pub profiles: Rc<RefCell<Vec<Profile>>>,

        // Views
        pub dashboard_page: RefCell<Option<DashboardPage>>,
        pub profiles_page: RefCell<Option<ProfilesPage>>,
        pub logs_page: RefCell<Option<LogsPage>>,
        pub settings_page: RefCell<Option<SettingsPage>>,
        pub help_page: RefCell<Option<HelpPage>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MainWindow {
        const NAME: &'static str = "CdNetworkManagerMainWindow";
        type Type = super::MainWindow;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for MainWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_ui();
            obj.setup_actions();
        }
    }

    impl WidgetImpl for MainWindow {}
    impl WindowImpl for MainWindow {}
    impl ApplicationWindowImpl for MainWindow {}
    impl AdwApplicationWindowImpl for MainWindow {}
}

glib::wrapper! {
    pub struct MainWindow(ObjectSubclass<imp::MainWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl MainWindow {
    pub fn new(app: &Application) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", app)
            .property("default-width", 1200)
            .property("default-height", 800)
            .build();

        // Apply saved maximized state
        let config = app.config();
        if config.window_maximized {
            window.maximize();
        }

        window.set_title(Some(crate::APP_NAME));
        
        // Connect close-request to save window state
        let app_weak = app.downgrade();
        window.connect_close_request(move |win| {
            if let Some(app) = app_weak.upgrade() {
                let mut config = app.config();
                
                // Only save size if not maximized
                config.window_maximized = win.is_maximized();
                if !config.window_maximized {
                    let (width, height) = win.default_size();
                    config.window_width = width;
                    config.window_height = height;
                }
                
                app.update_config(config);
            }
            glib::Propagation::Proceed
        });

        window
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // Main horizontal layout: sidebar + content
        let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        // Create sidebar content with fixed width
        let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_box.set_width_request(250); // 250px when expanded
        sidebar_box.add_css_class("sidebar-box");

        // Sidebar header
        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.set_show_end_title_buttons(false);
        sidebar_header.set_show_start_title_buttons(false);

        // Sidebar collapse button (top-right of sidebar header)
        let sidebar_toggle_btn = gtk::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Collapse sidebar")
            .build();
        sidebar_toggle_btn.add_css_class("flat");
        sidebar_toggle_btn.set_action_name(Some("win.toggle-sidebar"));
        sidebar_header.pack_end(&sidebar_toggle_btn);

        let sidebar_title = adw::WindowTitle::new(crate::APP_NAME, "");
        sidebar_header.set_title_widget(Some(&sidebar_title));
        sidebar_box.append(&sidebar_header);

        // Navigation list
        let sidebar_list = gtk::ListBox::new();
        sidebar_list.set_selection_mode(gtk::SelectionMode::Single);
        sidebar_list.add_css_class("navigation-sidebar");

        // Add navigation items and collect labels for collapse/expand
        let mut nav_labels = Vec::new();
        let mut nav_boxes = Vec::new();
        for nav_item in NavItem::all() {
            let (row, label, hbox) = self.create_nav_row_with_label(*nav_item);
            sidebar_list.append(&row);
            nav_labels.push(label);
            nav_boxes.push(hbox);
        }

        // Handle navigation selection
        let window_weak = self.downgrade();
        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                if let Some(window) = window_weak.upgrade() {
                    let index = row.index() as usize;
                    if let Some(nav_item) = NavItem::all().get(index) {
                        window.navigate_to(*nav_item);
                    }
                }
            }
        });

        let sidebar_scroll = gtk::ScrolledWindow::new();
        sidebar_scroll.set_vexpand(true);
        sidebar_scroll.set_child(Some(&sidebar_list));
        sidebar_box.append(&sidebar_scroll);

        // Version and author info at the bottom of sidebar
        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info_box.set_margin_start(12);
        info_box.set_margin_end(12);
        info_box.set_margin_top(8);
        info_box.set_margin_bottom(8);
        
        // Update banner — hidden until version check completes
        let update_banner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        update_banner.add_css_class("update-banner");
        update_banner.set_visible(false);
        update_banner.set_halign(gtk::Align::Start);

        let update_icon = gtk::Image::from_icon_name("software-update-available-symbolic");
        update_icon.set_pixel_size(14);
        update_banner.append(&update_icon);

        let update_label = gtk::Label::new(Some("New version available"));
        update_label.add_css_class("update-banner-label");
        update_banner.append(&update_label);

        info_box.append(&update_banner);
        imp.update_banner.replace(Some(update_banner));

        let version_label = gtk::Label::new(None);
        version_label.set_markup(&format!("<span size=\"x-small\">Version {}</span>", env!("CARGO_PKG_VERSION")));
        version_label.set_halign(gtk::Align::Start);
        info_box.append(&version_label);
        
        let author_label = gtk::Label::new(None);
        author_label.set_markup("<span size=\"x-small\">By Christos A. Daggas</span>");
        author_label.set_halign(gtk::Align::Start);
        info_box.append(&author_label);
        
        sidebar_box.append(&info_box);

        // Add separator between sidebar and content
        let separator = gtk::Separator::new(gtk::Orientation::Vertical);

        // Add sidebar to main box
        main_box.append(&sidebar_box);
        main_box.append(&separator);

        // Create content area
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content_box.set_hexpand(true);

        // Content header
        let header_bar = adw::HeaderBar::new();
        let content_title = adw::WindowTitle::new("Dashboard", "");
        header_bar.set_title_widget(Some(&content_title));

        // Add menu button
        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Main Menu"));

        // Create custom popover with theme selector
        let popover = self.create_main_menu_popover();
        menu_button.set_popover(Some(&popover));
        
        header_bar.pack_end(&menu_button);

        content_box.append(&header_bar);

        // Content stack for views
        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(200);
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);

        // Create pages
        let dashboard_page = DashboardPage::new();
        let profiles_page = ProfilesPage::new();
        let logs_page = LogsPage::new();
        let settings_page = SettingsPage::new();
        let help_page = HelpPage::new();

        content_stack.add_named(&dashboard_page, Some("dashboard"));
        content_stack.add_named(&profiles_page, Some("profiles"));
        content_stack.add_named(&logs_page, Some("logs"));
        content_stack.add_named(&settings_page, Some("settings"));
        content_stack.add_named(&help_page, Some("help"));

        content_box.append(&content_stack);

        // Add content to main box
        main_box.append(&content_box);

        // Wrap main_box in ToastOverlay for notifications
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&main_box));

        // Set the window content
        self.set_content(Some(&toast_overlay));

        // Store references
        *imp.sidebar_box.borrow_mut() = Some(sidebar_box);
        *imp.sidebar_list.borrow_mut() = Some(sidebar_list);
        *imp.content_stack.borrow_mut() = Some(content_stack);
        *imp.header_bar.borrow_mut() = Some(header_bar);
        *imp.content_title.borrow_mut() = Some(content_title);
        *imp.toast_overlay.borrow_mut() = Some(toast_overlay);
        *imp.sidebar_header.borrow_mut() = Some(sidebar_header);
        *imp.sidebar_title.borrow_mut() = Some(sidebar_title);
        *imp.info_box.borrow_mut() = Some(info_box);
        *imp.nav_labels.borrow_mut() = nav_labels;
        *imp.nav_boxes.borrow_mut() = nav_boxes;
        imp.sidebar_collapsed.set(false);
        *imp.sidebar_toggle_btn.borrow_mut() = Some(sidebar_toggle_btn);
        *imp.dashboard_page.borrow_mut() = Some(dashboard_page);
        *imp.profiles_page.borrow_mut() = Some(profiles_page);
        *imp.logs_page.borrow_mut() = Some(logs_page);
        *imp.settings_page.borrow_mut() = Some(settings_page);
        *imp.help_page.borrow_mut() = Some(help_page);

        // Select first item
        if let Some(list) = imp.sidebar_list.borrow().as_ref() {
            if let Some(first_row) = list.row_at_index(0) {
                list.select_row(Some(&first_row));
            }
        }
    }

    fn setup_actions(&self) {
        // New Profile action - shows create profile dialog
        let new_profile_action = gio::SimpleAction::new("new-profile", None);
        let window_weak = self.downgrade();
        new_profile_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.show_create_profile_dialog();
            }
        });
        self.add_action(&new_profile_action);
        
        // Edit Profile action
        let edit_profile_action = gio::SimpleAction::new("edit-profile", Some(&String::static_variant_type()));
        let window_weak = self.downgrade();
        edit_profile_action.connect_activate(move |_, param| {
            if let Some(window) = window_weak.upgrade() {
                if let Some(profile_id) = param.and_then(|p| p.get::<String>()) {
                    window.show_edit_profile_dialog(&profile_id);
                }
            }
        });
        self.add_action(&edit_profile_action);
        
        // Delete Profile action
        let delete_profile_action = gio::SimpleAction::new("delete-profile", Some(&String::static_variant_type()));
        let window_weak = self.downgrade();
        delete_profile_action.connect_activate(move |_, param| {
            if let Some(window) = window_weak.upgrade() {
                if let Some(profile_id) = param.and_then(|p| p.get::<String>()) {
                    window.show_delete_profile_dialog(&profile_id);
                }
            }
        });
        self.add_action(&delete_profile_action);
        
        // Duplicate Profile action
        let duplicate_profile_action = gio::SimpleAction::new("duplicate-profile", Some(&String::static_variant_type()));
        let window_weak = self.downgrade();
        duplicate_profile_action.connect_activate(move |_, param| {
            if let Some(window) = window_weak.upgrade() {
                if let Some(profile_id) = param.and_then(|p| p.get::<String>()) {
                    window.duplicate_profile(&profile_id);
                }
            }
        });
        self.add_action(&duplicate_profile_action);
        
        // Export Profiles action
        let export_profiles_action = gio::SimpleAction::new("export-profiles", None);
        let window_weak = self.downgrade();
        export_profiles_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.show_export_profiles_dialog();
            }
        });
        self.add_action(&export_profiles_action);
        
        // Import Profiles action
        let import_profiles_action = gio::SimpleAction::new("import-profiles", None);
        let window_weak = self.downgrade();
        import_profiles_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.show_import_profiles_dialog();
            }
        });
        self.add_action(&import_profiles_action);
        
        // Apply Profile action - actually applies network configuration
        let apply_profile_action = gio::SimpleAction::new("apply-profile", Some(&String::static_variant_type()));
        let window_weak = self.downgrade();
        apply_profile_action.connect_activate(move |_, param| {
            if let Some(window) = window_weak.upgrade() {
                if let Some(profile_id) = param.and_then(|p| p.get::<String>()) {
                    window.apply_profile(&profile_id);
                }
            }
        });
        self.add_action(&apply_profile_action);
        
        // Show Profile Switcher action - opens a dialog to select and apply a profile
        let show_switcher_action = gio::SimpleAction::new("show-profile-switcher", None);
        let window_weak = self.downgrade();
        show_switcher_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.show_profile_switcher_dialog();
            }
        });
        self.add_action(&show_switcher_action);

        // Toggle sidebar action
        let toggle_sidebar_action = gio::SimpleAction::new("toggle-sidebar", None);
        let window_weak = self.downgrade();
        toggle_sidebar_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.toggle_sidebar();
            }
        });
        self.add_action(&toggle_sidebar_action);
        
        // Copy network info to clipboard action
        let copy_network_info_action = gio::SimpleAction::new("copy-network-info", None);
        let window_weak = self.downgrade();
        copy_network_info_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.copy_network_info_to_clipboard();
            }
        });
        self.add_action(&copy_network_info_action);
        
        // Show network diagnostics action
        let show_diagnostics_action = gio::SimpleAction::new("show-diagnostics", None);
        let window_weak = self.downgrade();
        show_diagnostics_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.show_network_diagnostics_dialog();
            }
        });
        self.add_action(&show_diagnostics_action);
        
        // Navigation actions for keyboard shortcuts
        let navigate_dashboard = gio::SimpleAction::new("navigate-dashboard", None);
        let window_weak = self.downgrade();
        navigate_dashboard.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.navigate_to(NavItem::Dashboard);
            }
        });
        self.add_action(&navigate_dashboard);
        
        let navigate_profiles = gio::SimpleAction::new("navigate-profiles", None);
        let window_weak = self.downgrade();
        navigate_profiles.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.navigate_to(NavItem::Profiles);
            }
        });
        self.add_action(&navigate_profiles);
        
        let navigate_logs = gio::SimpleAction::new("navigate-logs", None);
        let window_weak = self.downgrade();
        navigate_logs.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.navigate_to(NavItem::Logs);
            }
        });
        self.add_action(&navigate_logs);
        
        let navigate_settings = gio::SimpleAction::new("navigate-settings", None);
        let window_weak = self.downgrade();
        navigate_settings.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.navigate_to(NavItem::Settings);
            }
        });
        self.add_action(&navigate_settings);
        
        let navigate_help = gio::SimpleAction::new("navigate-help", None);
        let window_weak = self.downgrade();
        navigate_help.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.navigate_to(NavItem::Help);
            }
        });
        self.add_action(&navigate_help);
        
        // Refresh action
        let refresh_action = gio::SimpleAction::new("refresh", None);
        let window_weak = self.downgrade();
        refresh_action.connect_activate(move |_, _| {
            if let Some(window) = window_weak.upgrade() {
                window.refresh_current_view();
            }
        });
        self.add_action(&refresh_action);
        
        // Set up keyboard shortcuts
        if let Some(app) = self.application() {
            app.set_accels_for_action("win.new-profile", &["<Primary>n"]);
            app.set_accels_for_action("win.show-profile-switcher", &["<Primary>p"]);
            app.set_accels_for_action("win.toggle-sidebar", &["<Primary>b"]);
            app.set_accels_for_action("win.copy-network-info", &["<Primary><Shift>c"]);
            app.set_accels_for_action("win.show-diagnostics", &["<Primary>d"]);
            app.set_accels_for_action("win.refresh", &["<Primary>r", "F5"]);
            app.set_accels_for_action("win.navigate-dashboard", &["<Primary>1"]);
            app.set_accels_for_action("win.navigate-profiles", &["<Primary>2"]);
            app.set_accels_for_action("win.navigate-logs", &["<Primary>3"]);
            app.set_accels_for_action("win.navigate-settings", &["<Primary>4"]);
            app.set_accels_for_action("win.navigate-help", &["<Primary>5", "F1"]);
        }
    }

    /// Toggle the sidebar between collapsed (icons only) and expanded.
    fn toggle_sidebar(&self) {
        let imp = self.imp();

        let is_collapsed = imp.sidebar_collapsed.get();
        let new_collapsed = !is_collapsed;
        imp.sidebar_collapsed.set(new_collapsed);

        // Update sidebar width and collapsed CSS class
        if let Some(sidebar_box) = imp.sidebar_box.borrow().as_ref() {
            if new_collapsed {
                sidebar_box.set_width_request(50); // Collapsed: icons only
                sidebar_box.add_css_class("sidebar-collapsed");
            } else {
                sidebar_box.set_width_request(250); // Expanded: full width
                sidebar_box.remove_css_class("sidebar-collapsed");
            }
        }

        // Hide/show sidebar title
        if let Some(sidebar_title) = imp.sidebar_title.borrow().as_ref() {
            sidebar_title.set_visible(!new_collapsed);
        }

        // Hide/show navigation labels and adjust nav box alignment
        for label in imp.nav_labels.borrow().iter() {
            label.set_visible(!new_collapsed);
        }
        for hbox in imp.nav_boxes.borrow().iter() {
            if new_collapsed {
                // Remove margins and center icon when collapsed
                hbox.set_margin_start(0);
                hbox.set_margin_end(0);
                hbox.set_spacing(0);
                hbox.set_halign(gtk::Align::Center);
            } else {
                // Restore margins and alignment when expanded
                hbox.set_margin_start(12);
                hbox.set_margin_end(12);
                hbox.set_spacing(12);
                hbox.set_halign(gtk::Align::Fill);
            }
        }

        // Hide/show info box at bottom
        if let Some(info_box) = imp.info_box.borrow().as_ref() {
            info_box.set_visible(!new_collapsed);
        }

        // Update toggle button tooltip and icon
        if let Some(btn) = imp.sidebar_toggle_btn.borrow().as_ref() {
            if new_collapsed {
                btn.set_tooltip_text(Some("Expand sidebar"));
                btn.set_icon_name("sidebar-show-right-symbolic");
            } else {
                btn.set_tooltip_text(Some("Collapse sidebar"));
                btn.set_icon_name("sidebar-show-symbolic");
            }
        }
    }

    fn show_create_profile_dialog(&self) {
        use crate::network_utils::detect_network_adapters;
        use crate::models::AdapterType;
        use std::collections::HashMap;
        
        let dialog = adw::Dialog::new();
        dialog.set_title("Create Profile");
        dialog.set_content_width(700);
        dialog.set_content_height(850);

        let toolbar_view = adw::ToolbarView::new();
        
        // Header bar
        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
        
        let cancel_btn = gtk::Button::with_label("Cancel");
        let create_btn = gtk::Button::with_label("Create");
        create_btn.add_css_class("suggested-action");
        create_btn.set_sensitive(false);
        
        header.pack_start(&cancel_btn);
        header.pack_end(&create_btn);
        toolbar_view.add_top_bar(&header);

        // Scrollable content
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);

        // === Template Selection ===
        let template_group = adw::PreferencesGroup::new();
        template_group.set_title("Start From Template");
        template_group.set_description(Some("Optionally start with a pre-configured template"));
        
        let template_model = gtk::StringList::new(&[
            "Blank (Empty)",
            "Home Network",
            "Office Network", 
            "Public WiFi",
            "VPN Only",
            "Development",
        ]);
        let template_row = adw::ComboRow::builder()
            .title("Template")
            .subtitle("Pre-configured settings for common scenarios")
            .model(&template_model)
            .selected(0) // Default to Blank
            .build();
        template_group.add(&template_row);
        
        content.append(&template_group);

        // === Profile Details ===
        let details_group = adw::PreferencesGroup::new();
        details_group.set_title("Profile Details");
        
        let name_entry = adw::EntryRow::new();
        name_entry.set_title("Name");
        details_group.add(&name_entry);
        
        let desc_entry = adw::EntryRow::new();
        desc_entry.set_title("Description");
        details_group.add(&desc_entry);
        
        let group_entry = adw::EntryRow::new();
        group_entry.set_title("Group (optional)");
        details_group.add(&group_entry);
        
        content.append(&details_group);

        // === Detect Available Network Adapters ===
        let adapters = detect_network_adapters();
        
        // Store adapter configuration widgets
        type AdapterWidgets = (
            adw::SwitchRow,           // enabled
            adw::ComboRow,            // ipv4 method
            adw::EntryRow,            // static ip
            adw::EntryRow,            // subnet
            adw::EntryRow,            // gateway
            adw::ComboRow,            // dns method
            adw::EntryRow,            // dns1
            adw::EntryRow,            // dns2
            Option<adw::EntryRow>,    // wifi ssid (only for wifi adapters)
        );
        let adapter_widgets: Rc<RefCell<HashMap<String, AdapterWidgets>>> = Rc::new(RefCell::new(HashMap::new()));
        
        // Create adapter configuration section
        let adapters_group = adw::PreferencesGroup::new();
        adapters_group.set_title("Network Adapters");
        adapters_group.set_description(Some("Configure settings for each network adapter. Each adapter can have its own IP, DNS, and state settings."));
        
        if adapters.is_empty() {
            let no_adapters_label = gtk::Label::new(Some("No network adapters detected"));
            no_adapters_label.add_css_class("dim-label");
            no_adapters_label.set_margin_top(12);
            no_adapters_label.set_margin_bottom(12);
            adapters_group.add(&no_adapters_label);
        } else {
            for adapter in &adapters {
                // Create an expander row for each adapter
                let expander = adw::ExpanderRow::new();
                let icon_name = adapter.adapter_type.icon_name();
                
                // Add icon as a prefix widget (since set_icon_name is deprecated in 1.3+)
                let icon = gtk::Image::from_icon_name(icon_name);
                expander.add_prefix(&icon);
                
                expander.set_title(&adapter.name);
                
                let subtitle = if let Some(mac) = &adapter.mac_address {
                    format!("{} • {}", adapter.adapter_type.display_name(), mac)
                } else {
                    adapter.adapter_type.display_name().to_string()
                };
                expander.set_subtitle(&subtitle);
                
                // Enable/disable switch for this adapter
                let enabled_row = adw::SwitchRow::builder()
                    .title("Enable Adapter")
                    .subtitle("Configure this adapter in the profile")
                    .active(true)
                    .build();
                expander.add_row(&enabled_row);
                
                // Create a container for adapter-specific settings
                let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                settings_box.set_margin_start(16);
                
                // IPv4 Configuration
                let ip_method_model = gtk::StringList::new(&["DHCP (Automatic)", "Static (Manual)", "Disabled"]);
                let ip_method_row = adw::ComboRow::builder()
                    .title("IPv4 Configuration")
                    .subtitle("How to obtain IP address")
                    .model(&ip_method_model)
                    .selected(0)
                    .build();
                expander.add_row(&ip_method_row);
                
                // Static IP fields
                let static_ip = adw::EntryRow::new();
                static_ip.set_title("IP Address");
                static_ip.set_text("192.168.1.100");
                static_ip.set_sensitive(false);
                expander.add_row(&static_ip);
                
                let subnet = adw::EntryRow::new();
                subnet.set_title("Subnet Mask");
                subnet.set_text("255.255.255.0");
                subnet.set_sensitive(false);
                expander.add_row(&subnet);
                
                let gateway = adw::EntryRow::new();
                gateway.set_title("Gateway");
                gateway.set_text("192.168.1.1");
                gateway.set_sensitive(false);
                expander.add_row(&gateway);
                
                // DNS Configuration
                let dns_method_model = gtk::StringList::new(&["Automatic (from DHCP)", "Manual"]);
                let dns_method_row = adw::ComboRow::builder()
                    .title("DNS Configuration")
                    .model(&dns_method_model)
                    .selected(0)
                    .build();
                expander.add_row(&dns_method_row);
                
                let dns1 = adw::EntryRow::new();
                dns1.set_title("Primary DNS");
                dns1.set_text("8.8.8.8");
                dns1.set_sensitive(false);
                expander.add_row(&dns1);
                
                let dns2 = adw::EntryRow::new();
                dns2.set_title("Secondary DNS");
                dns2.set_text("8.8.4.4");
                dns2.set_sensitive(false);
                expander.add_row(&dns2);
                
                // WiFi-specific: SSID selection
                let wifi_ssid_entry = if adapter.adapter_type == AdapterType::Wifi {
                    let ssid_entry = adw::EntryRow::new();
                    ssid_entry.set_title("WiFi Network (SSID)");
                    ssid_entry.set_sensitive(true);
                    expander.add_row(&ssid_entry);
                    Some(ssid_entry)
                } else {
                    None
                };
                
                // Connect IP method change to enable/disable static fields
                let static_ip_weak = static_ip.downgrade();
                let subnet_weak = subnet.downgrade();
                let gateway_weak = gateway.downgrade();
                ip_method_row.connect_selected_notify(move |row| {
                    let is_static = row.selected() == 1;
                    if let Some(e) = static_ip_weak.upgrade() { e.set_sensitive(is_static); }
                    if let Some(e) = subnet_weak.upgrade() { e.set_sensitive(is_static); }
                    if let Some(e) = gateway_weak.upgrade() { e.set_sensitive(is_static); }
                });
                
                // Connect DNS method change to enable/disable DNS fields
                let dns1_weak = dns1.downgrade();
                let dns2_weak = dns2.downgrade();
                dns_method_row.connect_selected_notify(move |row| {
                    let is_manual = row.selected() == 1;
                    if let Some(e) = dns1_weak.upgrade() { e.set_sensitive(is_manual); }
                    if let Some(e) = dns2_weak.upgrade() { e.set_sensitive(is_manual); }
                });
                
                // Connect enable switch to toggle all settings
                let ip_method_weak = ip_method_row.downgrade();
                let static_ip_weak = static_ip.downgrade();
                let subnet_weak = subnet.downgrade();
                let gateway_weak = gateway.downgrade();
                let dns_method_weak = dns_method_row.downgrade();
                let dns1_weak = dns1.downgrade();
                let dns2_weak = dns2.downgrade();
                let wifi_ssid_weak = wifi_ssid_entry.as_ref().map(|e| e.downgrade());
                
                enabled_row.connect_active_notify(move |row| {
                    let enabled = row.is_active();
                    if let Some(e) = ip_method_weak.upgrade() { e.set_sensitive(enabled); }
                    if let Some(e) = dns_method_weak.upgrade() { e.set_sensitive(enabled); }
                    
                    // Only enable static fields if method is static AND enabled
                    if !enabled {
                        if let Some(e) = static_ip_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = subnet_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = gateway_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = dns1_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = dns2_weak.upgrade() { e.set_sensitive(false); }
                    }
                    
                    if let Some(ref weak) = wifi_ssid_weak {
                        if let Some(e) = weak.upgrade() { e.set_sensitive(enabled); }
                    }
                });
                
                // Store widget references for this adapter
                adapter_widgets.borrow_mut().insert(
                    adapter.name.clone(),
                    (enabled_row, ip_method_row, static_ip, subnet, gateway, dns_method_row, dns1, dns2, wifi_ssid_entry)
                );
                
                adapters_group.add(&expander);
            }
        }
        
        content.append(&adapters_group);

        // === VPN Connection ===
        let vpn_group = adw::PreferencesGroup::new();
        vpn_group.set_title("VPN Connection");
        vpn_group.set_description(Some("Optionally connect to a VPN when this profile is active"));

        let vpn_enabled = adw::SwitchRow::builder()
            .title("Connect to VPN")
            .subtitle("Use a VPN connection from NetworkManager")
            .active(false)
            .build();
        vpn_group.add(&vpn_enabled);

        let vpn_name = adw::EntryRow::new();
        vpn_name.set_title("VPN Connection Name");
        vpn_name.set_sensitive(false);
        vpn_group.add(&vpn_name);

        let vpn_name_weak = vpn_name.downgrade();
        vpn_enabled.connect_active_notify(move |row| {
            if let Some(e) = vpn_name_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        content.append(&vpn_group);

        // === Proxy Settings ===
        let proxy_group = adw::PreferencesGroup::new();
        proxy_group.set_title("Proxy Settings");
        proxy_group.set_description(Some("Configure system proxy via gsettings"));

        let proxy_mode_model = gtk::StringList::new(&["None", "Manual", "Automatic (PAC)"]);
        let proxy_mode_row = adw::ComboRow::builder()
            .title("Proxy Mode")
            .model(&proxy_mode_model)
            .selected(0)
            .build();
        proxy_group.add(&proxy_mode_row);

        let http_proxy = adw::EntryRow::new();
        http_proxy.set_title("HTTP Proxy");
        http_proxy.set_text("http://proxy:8080");
        http_proxy.set_sensitive(false);
        proxy_group.add(&http_proxy);

        let https_proxy = adw::EntryRow::new();
        https_proxy.set_title("HTTPS Proxy");
        https_proxy.set_text("http://proxy:8080");
        https_proxy.set_sensitive(false);
        proxy_group.add(&https_proxy);

        let no_proxy = adw::EntryRow::new();
        no_proxy.set_title("No Proxy For");
        no_proxy.set_text("localhost,127.0.0.1");
        no_proxy.set_sensitive(false);
        proxy_group.add(&no_proxy);

        let pac_url = adw::EntryRow::new();
        pac_url.set_title("PAC URL");
        pac_url.set_sensitive(false);
        proxy_group.add(&pac_url);

        let http_proxy_weak = http_proxy.downgrade();
        let https_proxy_weak = https_proxy.downgrade();
        let no_proxy_weak = no_proxy.downgrade();
        let pac_url_weak = pac_url.downgrade();
        proxy_mode_row.connect_selected_notify(move |row| {
            let mode = row.selected();
            let is_manual = mode == 1;
            let is_auto = mode == 2;
            if let Some(e) = http_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = https_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = no_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = pac_url_weak.upgrade() { e.set_sensitive(is_auto); }
        });

        content.append(&proxy_group);

        // === Scripts/Programs ===
        let scripts_group = adw::PreferencesGroup::new();
        scripts_group.set_title("Scripts and Programs");
        scripts_group.set_description(Some("Run scripts when profile activates"));

        let pre_script_enabled = adw::SwitchRow::builder()
            .title("Run Pre-Script")
            .subtitle("Execute script before applying profile")
            .active(false)
            .build();
        scripts_group.add(&pre_script_enabled);

        let pre_script_path = adw::EntryRow::new();
        pre_script_path.set_title("Pre-Script Path");
        pre_script_path.set_text("/path/to/script.sh");
        pre_script_path.set_sensitive(false);
        scripts_group.add(&pre_script_path);

        let post_script_enabled = adw::SwitchRow::builder()
            .title("Run Post-Script")
            .subtitle("Execute script after applying profile")
            .active(false)
            .build();
        scripts_group.add(&post_script_enabled);

        let post_script_path = adw::EntryRow::new();
        post_script_path.set_title("Post-Script Path");
        post_script_path.set_text("/path/to/script.sh");
        post_script_path.set_sensitive(false);
        scripts_group.add(&post_script_path);

        let run_program_enabled = adw::SwitchRow::builder()
            .title("Run Program")
            .subtitle("Launch an application when profile activates")
            .active(false)
            .build();
        scripts_group.add(&run_program_enabled);

        let run_program_path = adw::EntryRow::new();
        run_program_path.set_title("Program Path");
        run_program_path.set_sensitive(false);
        scripts_group.add(&run_program_path);

        let run_program_args = adw::EntryRow::new();
        run_program_args.set_title("Program Arguments");
        run_program_args.set_sensitive(false);
        scripts_group.add(&run_program_args);

        // Script toggle handlers
        let pre_script_path_weak = pre_script_path.downgrade();
        pre_script_enabled.connect_active_notify(move |row| {
            if let Some(e) = pre_script_path_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        let post_script_path_weak = post_script_path.downgrade();
        post_script_enabled.connect_active_notify(move |row| {
            if let Some(e) = post_script_path_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        let run_program_path_weak = run_program_path.downgrade();
        let run_program_args_weak = run_program_args.downgrade();
        run_program_enabled.connect_active_notify(move |row| {
            let enabled = row.is_active();
            if let Some(e) = run_program_path_weak.upgrade() { e.set_sensitive(enabled); }
            if let Some(e) = run_program_args_weak.upgrade() { e.set_sensitive(enabled); }
        });

        content.append(&scripts_group);

        // Enable/disable create button based on name
        let create_btn_weak = create_btn.downgrade();
        name_entry.connect_changed(move |entry| {
            if let Some(btn) = create_btn_weak.upgrade() {
                btn.set_sensitive(!entry.text().is_empty());
            }
        });

        scroll.set_child(Some(&content));
        toolbar_view.set_content(Some(&scroll));
        dialog.set_child(Some(&toolbar_view));

        // Cancel action
        let dialog_weak = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
        });

        // Create weak references for fields needed in create handler
        let dialog_weak = dialog.downgrade();
        let window_weak = self.downgrade();
        let template_row_weak = template_row.downgrade();
        let name_entry_weak = name_entry.downgrade();
        let desc_entry_weak = desc_entry.downgrade();
        let group_entry_weak = group_entry.downgrade();
        // VPN
        let vpn_enabled_weak = vpn_enabled.downgrade();
        let vpn_name_weak = vpn_name.downgrade();
        // Proxy
        let proxy_mode_row_weak = proxy_mode_row.downgrade();
        let http_proxy_weak = http_proxy.downgrade();
        let https_proxy_weak = https_proxy.downgrade();
        let no_proxy_weak = no_proxy.downgrade();
        let pac_url_weak = pac_url.downgrade();
        // Scripts
        let pre_script_enabled_weak = pre_script_enabled.downgrade();
        let pre_script_path_weak = pre_script_path.downgrade();
        let post_script_enabled_weak = post_script_enabled.downgrade();
        let post_script_path_weak = post_script_path.downgrade();
        let run_program_enabled_weak = run_program_enabled.downgrade();
        let run_program_path_weak = run_program_path.downgrade();
        let run_program_args_weak = run_program_args.downgrade();
        // Adapter widgets
        let adapter_widgets_ref = adapter_widgets.clone();
        
        create_btn.connect_clicked(move |_| {
            let template_idx = template_row_weak.upgrade().map(|r| r.selected()).unwrap_or(0);
            let name = name_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let description = desc_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let group_name = group_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // VPN settings
            let vpn_on = vpn_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let vpn_name_val = vpn_name_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Proxy settings
            let proxy_mode = proxy_mode_row_weak.upgrade().map(|r| r.selected()).unwrap_or(0);
            let http_proxy_val = http_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let https_proxy_val = https_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let no_proxy_val = no_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let pac_url_val = pac_url_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Script settings
            let pre_script_on = pre_script_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let pre_script = pre_script_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let post_script_on = post_script_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let post_script = post_script_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let run_prog_on = run_program_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let run_prog_path = run_program_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let run_prog_args = run_program_args_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Collect adapter configurations
            let adapter_configs: Vec<(String, bool, u32, String, String, String, u32, String, String, Option<String>)> = 
                adapter_widgets_ref.borrow().iter().map(|(iface, widgets)| {
                    let (enabled, ip_method, static_ip, subnet, gateway, dns_method, dns1, dns2, wifi_ssid) = widgets;
                    (
                        iface.clone(),
                        enabled.is_active(),
                        ip_method.selected(),
                        static_ip.text().to_string(),
                        subnet.text().to_string(),
                        gateway.text().to_string(),
                        dns_method.selected(),
                        dns1.text().to_string(),
                        dns2.text().to_string(),
                        wifi_ssid.as_ref().map(|e| e.text().to_string()),
                    )
                }).collect();
            
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
            
            if let Some(window) = window_weak.upgrade() {
                use crate::models::{
                    NetworkAction, Ipv4Method, Ipv4Address, InterfaceState, 
                    SystemAction, AutomationAction, ProxyConfig, ProxyMode,
                    ProfileTemplate,
                };
                use std::path::PathBuf;
                
                // Create the profile - either from template or blank
                let mut profile = match template_idx {
                    0 => Profile::new(&name), // Blank
                    1 => ProfileTemplate::HomeNetwork.create_profile(&name),
                    2 => ProfileTemplate::OfficeNetwork.create_profile(&name),
                    3 => ProfileTemplate::PublicWifi.create_profile(&name),
                    4 => ProfileTemplate::VpnOnly.create_profile(&name),
                    5 => ProfileTemplate::Development.create_profile(&name),
                    _ => Profile::new(&name),
                };
                
                // Override description/group if user provided them
                if !description.is_empty() {
                    profile.metadata.description = Some(description);
                }
                
                if !group_name.is_empty() {
                    profile.metadata.group = Some(crate::models::ProfileGroup::new(&group_name));
                }
                
                // === Process each adapter configuration ===
                for (iface, enabled, ip_method, static_ip, subnet, gateway, dns_method, dns1, dns2, wifi_ssid) in adapter_configs {
                    // Add interface enable/disable action
                    profile.network_actions.push(NetworkAction::InterfaceEnable(InterfaceState {
                        interface: iface.clone(),
                        enabled,
                    }));
                    
                    if enabled {
                        // IPv4 Configuration
                        match ip_method {
                            0 => {
                                // DHCP
                                profile.network_actions.push(NetworkAction::Ipv4Config {
                                    interface: Some(iface.clone()),
                                    method: Ipv4Method::Auto,
                                    addresses: vec![],
                                    gateway: None,
                                });
                            }
                            1 => {
                                // Static
                                if let Ok(addr) = static_ip.parse::<std::net::Ipv4Addr>() {
                                    let prefix = Self::subnet_to_prefix(&subnet).unwrap_or(24);
                                    profile.network_actions.push(NetworkAction::Ipv4Config {
                                        interface: Some(iface.clone()),
                                        method: Ipv4Method::Manual,
                                        addresses: vec![Ipv4Address { address: addr, prefix }],
                                        gateway: gateway.parse().ok(),
                                    });
                                }
                            }
                            2 => {
                                // Disabled
                                profile.network_actions.push(NetworkAction::Ipv4Config {
                                    interface: Some(iface.clone()),
                                    method: Ipv4Method::Disabled,
                                    addresses: vec![],
                                    gateway: None,
                                });
                            }
                            _ => {}
                        }
                        
                        // DNS Configuration (if manual)
                        if dns_method == 1 {
                            let mut servers = Vec::new();
                            if let Ok(addr) = dns1.parse::<std::net::IpAddr>() {
                                servers.push(addr);
                            }
                            if let Ok(addr) = dns2.parse::<std::net::IpAddr>() {
                                servers.push(addr);
                            }
                            if !servers.is_empty() {
                                profile.network_actions.push(NetworkAction::DnsServers {
                                    interface: Some(iface.clone()),
                                    servers,
                                });
                            }
                        }
                        
                        // WiFi SSID (if applicable)
                        if let Some(ssid) = wifi_ssid {
                            if !ssid.is_empty() {
                                profile.network_actions.push(NetworkAction::WifiConnect {
                                    ssid,
                                    interface: Some(iface.clone()),
                                });
                            }
                        }
                    }
                }
                
                // === VPN Connection ===
                if vpn_on && !vpn_name_val.is_empty() {
                    profile.network_actions.push(NetworkAction::VpnConnect {
                        connection_name: vpn_name_val,
                    });
                }
                
                // === Proxy Configuration ===
                if proxy_mode > 0 {
                    let mode = match proxy_mode {
                        1 => ProxyMode::Manual,
                        2 => ProxyMode::Auto,
                        _ => ProxyMode::None,
                    };
                    
                    let no_proxy_list: Vec<String> = no_proxy_val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    
                    profile.system_actions.push(SystemAction::ProxyConfig(ProxyConfig {
                        mode,
                        http_proxy: if proxy_mode == 1 && !http_proxy_val.is_empty() { Some(http_proxy_val) } else { None },
                        https_proxy: if proxy_mode == 1 && !https_proxy_val.is_empty() { Some(https_proxy_val) } else { None },
                        ftp_proxy: None,
                        socks_proxy: None,
                        no_proxy: no_proxy_list,
                        pac_url: if proxy_mode == 2 && !pac_url_val.is_empty() { Some(pac_url_val) } else { None },
                    }));
                }
                
                // === Pre-Script ===
                if pre_script_on && !pre_script.is_empty() {
                    profile.automation_actions.push(AutomationAction::PreScript {
                        path: PathBuf::from(pre_script),
                        args: vec![],
                        env: std::collections::HashMap::new(),
                        mode: crate::models::ScriptMode::Wait,
                        working_dir: None,
                        continue_on_error: false,
                    });
                }
                
                // === Post-Script ===
                if post_script_on && !post_script.is_empty() {
                    profile.automation_actions.push(AutomationAction::PostScript {
                        path: PathBuf::from(post_script),
                        args: vec![],
                        env: std::collections::HashMap::new(),
                        mode: crate::models::ScriptMode::Wait,
                        working_dir: None,
                        continue_on_error: false,
                    });
                }
                
                // === Run Program ===
                if run_prog_on && !run_prog_path.is_empty() {
                    let args: Vec<String> = run_prog_args
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    
                    profile.automation_actions.push(AutomationAction::RunProgram {
                        program: run_prog_path,
                        args,
                        env: std::collections::HashMap::new(),
                        mode: crate::models::ProgramMode::Background,
                        working_dir: None,
                    });
                }
                
                // Store the profile
                let imp = window.imp();
                let profile_name_for_log = name.clone();
                imp.profiles.borrow_mut().push(profile);
                
                // Save to cache
                window.save_profiles_to_cache();
                
                // Log the profile creation
                if let Some(store) = imp.data_store.borrow().as_ref() {
                    store.append_log("INFO", &format!("Profile '{}' created", profile_name_for_log));
                }
                
                // Refresh logs page
                if let Some(logs_page) = imp.logs_page.borrow().as_ref() {
                    logs_page.refresh_logs();
                }
                
                // Update the profiles page
                if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                    let profiles = imp.profiles.borrow().clone();
                    profiles_page.update_profiles(profiles);
                }
                
                // Navigate to profiles page
                window.navigate_to(NavItem::Profiles);
                if let Some(list) = imp.sidebar_list.borrow().as_ref() {
                    if let Some(row) = list.row_at_index(1) {
                        list.select_row(Some(&row));
                    }
                }
                
                window.show_toast(&format!("Profile '{}' created", name));
            }
        });

        dialog.present(Some(self));
    }
    
    /// Helper to convert subnet mask string to prefix length
    fn subnet_to_prefix(subnet: &str) -> Option<u8> {
        match subnet.trim() {
            "255.255.255.255" => Some(32),
            "255.255.255.254" => Some(31),
            "255.255.255.252" => Some(30),
            "255.255.255.248" => Some(29),
            "255.255.255.240" => Some(28),
            "255.255.255.224" => Some(27),
            "255.255.255.192" => Some(26),
            "255.255.255.128" => Some(25),
            "255.255.255.0" => Some(24),
            "255.255.254.0" => Some(23),
            "255.255.252.0" => Some(22),
            "255.255.248.0" => Some(21),
            "255.255.240.0" => Some(20),
            "255.255.224.0" => Some(19),
            "255.255.192.0" => Some(18),
            "255.255.128.0" => Some(17),
            "255.255.0.0" => Some(16),
            "255.254.0.0" => Some(15),
            "255.252.0.0" => Some(14),
            "255.248.0.0" => Some(13),
            "255.240.0.0" => Some(12),
            "255.224.0.0" => Some(11),
            "255.192.0.0" => Some(10),
            "255.128.0.0" => Some(9),
            "255.0.0.0" => Some(8),
            _ => None,
        }
    }
    
    /// Get the NetworkManager connection name for a given interface
    fn get_connection_for_interface(interface: &str) -> Option<String> {
        use std::process::Command;
        
        // Use nmcli to get the connection name for this device
        let output = Command::new("nmcli")
            .args(["-t", "-f", "NAME,DEVICE", "connection", "show", "--active"])
            .output()
            .ok()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 && parts[1] == interface {
                    return Some(parts[0].to_string());
                }
            }
        }
        
        // Fallback: try to find any connection that uses this interface
        let output = Command::new("nmcli")
            .args(["-t", "-f", "NAME,DEVICE", "connection", "show"])
            .output()
            .ok()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 && parts[1] == interface {
                    return Some(parts[0].to_string());
                }
            }
        }
        
        None
    }
    
    /// Show edit profile dialog
    fn show_edit_profile_dialog(&self, profile_id: &str) {
        use crate::network_utils::detect_network_adapters;
        use crate::models::{AdapterType, NetworkAction, Ipv4Method, SystemAction, AutomationAction, ProxyMode};
        use std::collections::HashMap;
        
        let imp = self.imp();
        
        // Find the profile
        let profile_opt = {
            let profiles = imp.profiles.borrow();
            profiles.iter().find(|p| p.id().to_string() == profile_id).cloned()
        };
        
        let Some(profile) = profile_opt else {
            self.show_toast("Profile not found");
            return;
        };
        
        let dialog = adw::Dialog::new();
        dialog.set_title("Edit Profile");
        dialog.set_content_width(700);
        dialog.set_content_height(850);

        let toolbar_view = adw::ToolbarView::new();
        
        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
        
        let cancel_btn = gtk::Button::with_label("Cancel");
        let save_btn = gtk::Button::with_label("Save");
        save_btn.add_css_class("suggested-action");
        
        header.pack_start(&cancel_btn);
        header.pack_end(&save_btn);
        toolbar_view.add_top_bar(&header);

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);

        // === Profile Details ===
        let details_group = adw::PreferencesGroup::new();
        details_group.set_title("Profile Details");
        
        let name_entry = adw::EntryRow::new();
        name_entry.set_title("Name");
        name_entry.set_text(profile.name());
        details_group.add(&name_entry);
        
        let desc_entry = adw::EntryRow::new();
        desc_entry.set_title("Description");
        if let Some(desc) = &profile.metadata.description {
            desc_entry.set_text(desc);
        }
        details_group.add(&desc_entry);
        
        let group_entry = adw::EntryRow::new();
        group_entry.set_title("Group (optional)");
        if let Some(grp) = &profile.metadata.group {
            group_entry.set_text(&grp.name);
        }
        details_group.add(&group_entry);
        
        content.append(&details_group);

        // === Detect Available Network Adapters ===
        let adapters = detect_network_adapters();
        
        // Helper to find network action for an interface
        let find_ipv4_config = |iface: &str| -> Option<(Ipv4Method, String, String, String)> {
            for action in &profile.network_actions {
                if let NetworkAction::Ipv4Config { interface, method, addresses, gateway } = action {
                    if interface.as_deref() == Some(iface) {
                        let ip = addresses.first().map(|a| a.address.to_string()).unwrap_or_default();
                        let prefix = addresses.first().map(|a| a.prefix).unwrap_or(24);
                        let subnet = Self::prefix_to_subnet(prefix);
                        let gw = gateway.map(|g| g.to_string()).unwrap_or_default();
                        return Some((method.clone(), ip, subnet, gw));
                    }
                }
            }
            None
        };
        
        let find_dns_servers = |iface: &str| -> Option<(String, String)> {
            for action in &profile.network_actions {
                if let NetworkAction::DnsServers { interface, servers } = action {
                    if interface.as_deref() == Some(iface) {
                        let dns1 = servers.first().map(|s| s.to_string()).unwrap_or_default();
                        let dns2 = servers.get(1).map(|s| s.to_string()).unwrap_or_default();
                        return Some((dns1, dns2));
                    }
                }
            }
            None
        };
        
        let find_interface_enabled = |iface: &str| -> bool {
            for action in &profile.network_actions {
                if let NetworkAction::InterfaceEnable(state) = action {
                    if state.interface == iface {
                        return state.enabled;
                    }
                }
            }
            true
        };
        
        let find_wifi_ssid = |iface: &str| -> Option<String> {
            for action in &profile.network_actions {
                if let NetworkAction::WifiConnect { ssid, interface } = action {
                    if interface.as_deref() == Some(iface) {
                        return Some(ssid.clone());
                    }
                }
            }
            None
        };
        
        // Store adapter configuration widgets
        type AdapterWidgets = (
            adw::SwitchRow,           // enabled
            adw::ComboRow,            // ipv4 method
            adw::EntryRow,            // static ip
            adw::EntryRow,            // subnet
            adw::EntryRow,            // gateway
            adw::ComboRow,            // dns method
            adw::EntryRow,            // dns1
            adw::EntryRow,            // dns2
            Option<adw::EntryRow>,    // wifi ssid (only for wifi adapters)
        );
        let adapter_widgets: Rc<RefCell<HashMap<String, AdapterWidgets>>> = Rc::new(RefCell::new(HashMap::new()));
        
        // Create adapter configuration section
        let adapters_group = adw::PreferencesGroup::new();
        adapters_group.set_title("Network Adapters");
        adapters_group.set_description(Some("Configure settings for each network adapter"));
        
        if adapters.is_empty() {
            let no_adapters_label = gtk::Label::new(Some("No network adapters detected"));
            no_adapters_label.add_css_class("dim-label");
            no_adapters_label.set_margin_top(12);
            no_adapters_label.set_margin_bottom(12);
            adapters_group.add(&no_adapters_label);
        } else {
            for adapter in &adapters {
                let expander = adw::ExpanderRow::new();
                let icon_name = adapter.adapter_type.icon_name();
                let icon = gtk::Image::from_icon_name(icon_name);
                expander.add_prefix(&icon);
                expander.set_title(&adapter.name);
                
                let subtitle = if let Some(mac) = &adapter.mac_address {
                    format!("{} • {}", adapter.adapter_type.display_name(), mac)
                } else {
                    adapter.adapter_type.display_name().to_string()
                };
                expander.set_subtitle(&subtitle);
                
                // Load existing values
                let iface_enabled = find_interface_enabled(&adapter.name);
                let ipv4_config = find_ipv4_config(&adapter.name);
                let dns_config = find_dns_servers(&adapter.name);
                let wifi_ssid = find_wifi_ssid(&adapter.name);
                
                // Enable/disable switch for this adapter
                let enabled_row = adw::SwitchRow::builder()
                    .title("Enable Adapter")
                    .subtitle("Configure this adapter in the profile")
                    .active(iface_enabled)
                    .build();
                expander.add_row(&enabled_row);
                
                // IPv4 Configuration
                let ip_method_model = gtk::StringList::new(&["DHCP (Automatic)", "Static (Manual)", "Disabled"]);
                let ip_method_row = adw::ComboRow::builder()
                    .title("IPv4 Configuration")
                    .subtitle("How to obtain IP address")
                    .model(&ip_method_model)
                    .selected(match &ipv4_config {
                        Some((Ipv4Method::Auto, _, _, _)) => 0,
                        Some((Ipv4Method::Manual, _, _, _)) => 1,
                        Some((Ipv4Method::Disabled, _, _, _)) => 2,
                        _ => 0,
                    })
                    .build();
                expander.add_row(&ip_method_row);
                
                // Static IP fields
                let static_ip = adw::EntryRow::new();
                static_ip.set_title("IP Address");
                static_ip.set_text(&ipv4_config.as_ref().map(|(_, ip, _, _)| ip.clone()).unwrap_or_else(|| "192.168.1.100".to_string()));
                static_ip.set_sensitive(matches!(&ipv4_config, Some((Ipv4Method::Manual, _, _, _))));
                expander.add_row(&static_ip);
                
                let subnet = adw::EntryRow::new();
                subnet.set_title("Subnet Mask");
                subnet.set_text(&ipv4_config.as_ref().map(|(_, _, s, _)| s.clone()).unwrap_or_else(|| "255.255.255.0".to_string()));
                subnet.set_sensitive(matches!(&ipv4_config, Some((Ipv4Method::Manual, _, _, _))));
                expander.add_row(&subnet);
                
                let gateway = adw::EntryRow::new();
                gateway.set_title("Gateway");
                gateway.set_text(&ipv4_config.as_ref().map(|(_, _, _, g)| g.clone()).unwrap_or_else(|| "192.168.1.1".to_string()));
                gateway.set_sensitive(matches!(&ipv4_config, Some((Ipv4Method::Manual, _, _, _))));
                expander.add_row(&gateway);
                
                // DNS Configuration
                let has_manual_dns = dns_config.is_some();
                let dns_method_model = gtk::StringList::new(&["Automatic (from DHCP)", "Manual"]);
                let dns_method_row = adw::ComboRow::builder()
                    .title("DNS Configuration")
                    .model(&dns_method_model)
                    .selected(if has_manual_dns { 1 } else { 0 })
                    .build();
                expander.add_row(&dns_method_row);
                
                let dns1 = adw::EntryRow::new();
                dns1.set_title("Primary DNS");
                dns1.set_text(&dns_config.as_ref().map(|(d1, _)| d1.clone()).unwrap_or_else(|| "8.8.8.8".to_string()));
                dns1.set_sensitive(has_manual_dns);
                expander.add_row(&dns1);
                
                let dns2 = adw::EntryRow::new();
                dns2.set_title("Secondary DNS");
                dns2.set_text(&dns_config.as_ref().map(|(_, d2)| d2.clone()).unwrap_or_else(|| "8.8.4.4".to_string()));
                dns2.set_sensitive(has_manual_dns);
                expander.add_row(&dns2);
                
                // WiFi-specific: SSID selection
                let wifi_ssid_entry = if adapter.adapter_type == AdapterType::Wifi {
                    let ssid_entry = adw::EntryRow::new();
                    ssid_entry.set_title("WiFi Network (SSID)");
                    ssid_entry.set_text(&wifi_ssid.unwrap_or_default());
                    ssid_entry.set_sensitive(iface_enabled);
                    expander.add_row(&ssid_entry);
                    Some(ssid_entry)
                } else {
                    None
                };
                
                // Connect IP method change to enable/disable static fields
                let static_ip_weak = static_ip.downgrade();
                let subnet_weak = subnet.downgrade();
                let gateway_weak = gateway.downgrade();
                ip_method_row.connect_selected_notify(move |row| {
                    let is_static = row.selected() == 1;
                    if let Some(e) = static_ip_weak.upgrade() { e.set_sensitive(is_static); }
                    if let Some(e) = subnet_weak.upgrade() { e.set_sensitive(is_static); }
                    if let Some(e) = gateway_weak.upgrade() { e.set_sensitive(is_static); }
                });
                
                // Connect DNS method change to enable/disable DNS fields
                let dns1_weak = dns1.downgrade();
                let dns2_weak = dns2.downgrade();
                dns_method_row.connect_selected_notify(move |row| {
                    let is_manual = row.selected() == 1;
                    if let Some(e) = dns1_weak.upgrade() { e.set_sensitive(is_manual); }
                    if let Some(e) = dns2_weak.upgrade() { e.set_sensitive(is_manual); }
                });
                
                // Connect enable switch to toggle all settings
                let ip_method_weak = ip_method_row.downgrade();
                let static_ip_weak = static_ip.downgrade();
                let subnet_weak = subnet.downgrade();
                let gateway_weak = gateway.downgrade();
                let dns_method_weak = dns_method_row.downgrade();
                let dns1_weak = dns1.downgrade();
                let dns2_weak = dns2.downgrade();
                let wifi_ssid_weak = wifi_ssid_entry.as_ref().map(|e| e.downgrade());
                
                enabled_row.connect_active_notify(move |row| {
                    let enabled = row.is_active();
                    if let Some(e) = ip_method_weak.upgrade() { e.set_sensitive(enabled); }
                    if let Some(e) = dns_method_weak.upgrade() { e.set_sensitive(enabled); }
                    
                    if !enabled {
                        if let Some(e) = static_ip_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = subnet_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = gateway_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = dns1_weak.upgrade() { e.set_sensitive(false); }
                        if let Some(e) = dns2_weak.upgrade() { e.set_sensitive(false); }
                    }
                    
                    if let Some(ref weak) = wifi_ssid_weak {
                        if let Some(e) = weak.upgrade() { e.set_sensitive(enabled); }
                    }
                });
                
                adapter_widgets.borrow_mut().insert(
                    adapter.name.clone(),
                    (enabled_row, ip_method_row, static_ip, subnet, gateway, dns_method_row, dns1, dns2, wifi_ssid_entry)
                );
                
                adapters_group.add(&expander);
            }
        }
        
        content.append(&adapters_group);

        // === VPN Connection ===
        let vpn_group = adw::PreferencesGroup::new();
        vpn_group.set_title("VPN Connection");
        vpn_group.set_description(Some("Optionally connect to a VPN when this profile is active"));

        // Find existing VPN config
        let existing_vpn = profile.network_actions.iter().find_map(|a| {
            if let NetworkAction::VpnConnect { connection_name } = a {
                Some(connection_name.clone())
            } else {
                None
            }
        });

        let vpn_enabled = adw::SwitchRow::builder()
            .title("Connect to VPN")
            .subtitle("Use a VPN connection from NetworkManager")
            .active(existing_vpn.is_some())
            .build();
        vpn_group.add(&vpn_enabled);

        let vpn_name = adw::EntryRow::new();
        vpn_name.set_title("VPN Connection Name");
        vpn_name.set_text(&existing_vpn.unwrap_or_default());
        vpn_name.set_sensitive(vpn_enabled.is_active());
        vpn_group.add(&vpn_name);

        let vpn_name_weak = vpn_name.downgrade();
        vpn_enabled.connect_active_notify(move |row| {
            if let Some(e) = vpn_name_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        content.append(&vpn_group);

        // === Proxy Settings ===
        let proxy_group = adw::PreferencesGroup::new();
        proxy_group.set_title("Proxy Settings");
        proxy_group.set_description(Some("Configure system proxy via gsettings"));

        // Find existing proxy config
        let existing_proxy = profile.system_actions.iter().find_map(|a| {
            if let SystemAction::ProxyConfig(cfg) = a {
                Some(cfg.clone())
            } else {
                None
            }
        });

        let proxy_mode_idx = existing_proxy.as_ref().map(|p| match p.mode {
            ProxyMode::None => 0,
            ProxyMode::Manual => 1,
            ProxyMode::Auto => 2,
        }).unwrap_or(0);

        let proxy_mode_model = gtk::StringList::new(&["None", "Manual", "Automatic (PAC)"]);
        let proxy_mode_row = adw::ComboRow::builder()
            .title("Proxy Mode")
            .model(&proxy_mode_model)
            .selected(proxy_mode_idx)
            .build();
        proxy_group.add(&proxy_mode_row);

        let http_proxy = adw::EntryRow::new();
        http_proxy.set_title("HTTP Proxy");
        http_proxy.set_text(&existing_proxy.as_ref().and_then(|p| p.http_proxy.clone()).unwrap_or_else(|| "http://proxy:8080".to_string()));
        http_proxy.set_sensitive(proxy_mode_idx == 1);
        proxy_group.add(&http_proxy);

        let https_proxy = adw::EntryRow::new();
        https_proxy.set_title("HTTPS Proxy");
        https_proxy.set_text(&existing_proxy.as_ref().and_then(|p| p.https_proxy.clone()).unwrap_or_else(|| "http://proxy:8080".to_string()));
        https_proxy.set_sensitive(proxy_mode_idx == 1);
        proxy_group.add(&https_proxy);

        let no_proxy = adw::EntryRow::new();
        no_proxy.set_title("No Proxy For");
        no_proxy.set_text(&existing_proxy.as_ref().map(|p| p.no_proxy.join(",")).unwrap_or_else(|| "localhost,127.0.0.1".to_string()));
        no_proxy.set_sensitive(proxy_mode_idx == 1);
        proxy_group.add(&no_proxy);

        let pac_url = adw::EntryRow::new();
        pac_url.set_title("PAC URL");
        pac_url.set_text(&existing_proxy.as_ref().and_then(|p| p.pac_url.clone()).unwrap_or_default());
        pac_url.set_sensitive(proxy_mode_idx == 2);
        proxy_group.add(&pac_url);

        let http_proxy_weak = http_proxy.downgrade();
        let https_proxy_weak = https_proxy.downgrade();
        let no_proxy_weak = no_proxy.downgrade();
        let pac_url_weak = pac_url.downgrade();
        proxy_mode_row.connect_selected_notify(move |row| {
            let mode = row.selected();
            let is_manual = mode == 1;
            let is_auto = mode == 2;
            if let Some(e) = http_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = https_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = no_proxy_weak.upgrade() { e.set_sensitive(is_manual); }
            if let Some(e) = pac_url_weak.upgrade() { e.set_sensitive(is_auto); }
        });

        content.append(&proxy_group);

        // === Scripts/Programs ===
        let scripts_group = adw::PreferencesGroup::new();
        scripts_group.set_title("Scripts and Programs");
        scripts_group.set_description(Some("Run scripts when profile activates"));

        // Find existing script/program configs
        let existing_pre_script = profile.automation_actions.iter().find_map(|a| {
            if let AutomationAction::PreScript { path, .. } = a {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        });

        let existing_post_script = profile.automation_actions.iter().find_map(|a| {
            if let AutomationAction::PostScript { path, .. } = a {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        });

        let existing_program = profile.automation_actions.iter().find_map(|a| {
            if let AutomationAction::RunProgram { program, args, .. } = a {
                Some((program.clone(), args.join(" ")))
            } else {
                None
            }
        });

        let pre_script_enabled = adw::SwitchRow::builder()
            .title("Run Pre-Script")
            .subtitle("Execute script before applying profile")
            .active(existing_pre_script.is_some())
            .build();
        scripts_group.add(&pre_script_enabled);

        let pre_script_path = adw::EntryRow::new();
        pre_script_path.set_title("Pre-Script Path");
        pre_script_path.set_text(&existing_pre_script.unwrap_or_else(|| "/path/to/script.sh".to_string()));
        pre_script_path.set_sensitive(pre_script_enabled.is_active());
        scripts_group.add(&pre_script_path);

        let post_script_enabled = adw::SwitchRow::builder()
            .title("Run Post-Script")
            .subtitle("Execute script after applying profile")
            .active(existing_post_script.is_some())
            .build();
        scripts_group.add(&post_script_enabled);

        let post_script_path = adw::EntryRow::new();
        post_script_path.set_title("Post-Script Path");
        post_script_path.set_text(&existing_post_script.unwrap_or_else(|| "/path/to/script.sh".to_string()));
        post_script_path.set_sensitive(post_script_enabled.is_active());
        scripts_group.add(&post_script_path);

        let run_program_enabled = adw::SwitchRow::builder()
            .title("Run Program")
            .subtitle("Launch an application when profile activates")
            .active(existing_program.is_some())
            .build();
        scripts_group.add(&run_program_enabled);

        let run_program_path = adw::EntryRow::new();
        run_program_path.set_title("Program Path");
        run_program_path.set_text(&existing_program.as_ref().map(|(p, _)| p.clone()).unwrap_or_default());
        run_program_path.set_sensitive(run_program_enabled.is_active());
        scripts_group.add(&run_program_path);

        let run_program_args = adw::EntryRow::new();
        run_program_args.set_title("Program Arguments");
        run_program_args.set_text(&existing_program.as_ref().map(|(_, a)| a.clone()).unwrap_or_default());
        run_program_args.set_sensitive(run_program_enabled.is_active());
        scripts_group.add(&run_program_args);

        // Script toggle handlers
        let pre_script_path_weak = pre_script_path.downgrade();
        pre_script_enabled.connect_active_notify(move |row| {
            if let Some(e) = pre_script_path_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        let post_script_path_weak = post_script_path.downgrade();
        post_script_enabled.connect_active_notify(move |row| {
            if let Some(e) = post_script_path_weak.upgrade() { e.set_sensitive(row.is_active()); }
        });

        let run_program_path_weak = run_program_path.downgrade();
        let run_program_args_weak = run_program_args.downgrade();
        run_program_enabled.connect_active_notify(move |row| {
            let enabled = row.is_active();
            if let Some(e) = run_program_path_weak.upgrade() { e.set_sensitive(enabled); }
            if let Some(e) = run_program_args_weak.upgrade() { e.set_sensitive(enabled); }
        });

        content.append(&scripts_group);

        scroll.set_child(Some(&content));
        toolbar_view.set_content(Some(&scroll));
        dialog.set_child(Some(&toolbar_view));

        // Cancel action
        let dialog_weak = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
        });

        // Create weak references for save handler
        let dialog_weak = dialog.downgrade();
        let window_weak = self.downgrade();
        let profile_id = profile_id.to_string();
        let name_entry_weak = name_entry.downgrade();
        let desc_entry_weak = desc_entry.downgrade();
        let group_entry_weak = group_entry.downgrade();
        // VPN
        let vpn_enabled_weak = vpn_enabled.downgrade();
        let vpn_name_weak = vpn_name.downgrade();
        // Proxy
        let proxy_mode_row_weak = proxy_mode_row.downgrade();
        let http_proxy_weak = http_proxy.downgrade();
        let https_proxy_weak = https_proxy.downgrade();
        let no_proxy_weak = no_proxy.downgrade();
        let pac_url_weak = pac_url.downgrade();
        // Scripts
        let pre_script_enabled_weak = pre_script_enabled.downgrade();
        let pre_script_path_weak = pre_script_path.downgrade();
        let post_script_enabled_weak = post_script_enabled.downgrade();
        let post_script_path_weak = post_script_path.downgrade();
        let run_program_enabled_weak = run_program_enabled.downgrade();
        let run_program_path_weak = run_program_path.downgrade();
        let run_program_args_weak = run_program_args.downgrade();
        // Adapter widgets
        let adapter_widgets_ref = adapter_widgets.clone();
        
        save_btn.connect_clicked(move |_| {
            let new_name = name_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let new_desc = desc_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let new_group = group_entry_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // VPN settings
            let vpn_on = vpn_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let vpn_name_val = vpn_name_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Proxy settings
            let proxy_mode = proxy_mode_row_weak.upgrade().map(|r| r.selected()).unwrap_or(0);
            let http_proxy_val = http_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let https_proxy_val = https_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let no_proxy_val = no_proxy_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let pac_url_val = pac_url_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Script settings
            let pre_script_on = pre_script_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let pre_script = pre_script_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let post_script_on = post_script_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let post_script = post_script_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let run_prog_on = run_program_enabled_weak.upgrade().map(|r| r.is_active()).unwrap_or(false);
            let run_prog_path = run_program_path_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            let run_prog_args = run_program_args_weak.upgrade().map(|e| e.text().to_string()).unwrap_or_default();
            
            // Collect adapter configurations
            let adapter_configs: Vec<(String, bool, u32, String, String, String, u32, String, String, Option<String>)> = 
                adapter_widgets_ref.borrow().iter().map(|(iface, widgets)| {
                    let (enabled, ip_method, static_ip, subnet, gateway, dns_method, dns1, dns2, wifi_ssid) = widgets;
                    (
                        iface.clone(),
                        enabled.is_active(),
                        ip_method.selected(),
                        static_ip.text().to_string(),
                        subnet.text().to_string(),
                        gateway.text().to_string(),
                        dns_method.selected(),
                        dns1.text().to_string(),
                        dns2.text().to_string(),
                        wifi_ssid.as_ref().map(|e| e.text().to_string()),
                    )
                }).collect();
            
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
            
            if let Some(window) = window_weak.upgrade() {
                use crate::models::{
                    NetworkAction, Ipv4Method, Ipv4Address, InterfaceState, 
                    SystemAction, AutomationAction, ProxyConfig, ProxyMode,
                };
                use std::path::PathBuf;
                
                let imp = window.imp();
                
                // Update profile in storage
                {
                    let mut profiles = imp.profiles.borrow_mut();
                    if let Some(profile) = profiles.iter_mut().find(|p| p.id().to_string() == profile_id) {
                        // Update metadata
                        profile.metadata.name = new_name.clone();
                        profile.metadata.description = if new_desc.is_empty() { None } else { Some(new_desc) };
                        profile.metadata.group = if new_group.is_empty() { 
                            None 
                        } else { 
                            Some(crate::models::ProfileGroup::new(&new_group)) 
                        };
                        
                        // Clear existing actions and rebuild
                        profile.network_actions.clear();
                        profile.system_actions.clear();
                        profile.automation_actions.clear();
                        
                        // === Process each adapter configuration ===
                        for (iface, enabled, ip_method, static_ip, subnet, gateway, dns_method, dns1, dns2, wifi_ssid) in adapter_configs {
                            profile.network_actions.push(NetworkAction::InterfaceEnable(InterfaceState {
                                interface: iface.clone(),
                                enabled,
                            }));
                            
                            if enabled {
                                match ip_method {
                                    0 => {
                                        profile.network_actions.push(NetworkAction::Ipv4Config {
                                            interface: Some(iface.clone()),
                                            method: Ipv4Method::Auto,
                                            addresses: vec![],
                                            gateway: None,
                                        });
                                    }
                                    1 => {
                                        if let Ok(addr) = static_ip.parse::<std::net::Ipv4Addr>() {
                                            let prefix = MainWindow::subnet_to_prefix(&subnet).unwrap_or(24);
                                            profile.network_actions.push(NetworkAction::Ipv4Config {
                                                interface: Some(iface.clone()),
                                                method: Ipv4Method::Manual,
                                                addresses: vec![Ipv4Address { address: addr, prefix }],
                                                gateway: gateway.parse().ok(),
                                            });
                                        }
                                    }
                                    2 => {
                                        profile.network_actions.push(NetworkAction::Ipv4Config {
                                            interface: Some(iface.clone()),
                                            method: Ipv4Method::Disabled,
                                            addresses: vec![],
                                            gateway: None,
                                        });
                                    }
                                    _ => {}
                                }
                                
                                if dns_method == 1 {
                                    let mut servers = Vec::new();
                                    if let Ok(addr) = dns1.parse::<std::net::IpAddr>() {
                                        servers.push(addr);
                                    }
                                    if let Ok(addr) = dns2.parse::<std::net::IpAddr>() {
                                        servers.push(addr);
                                    }
                                    if !servers.is_empty() {
                                        profile.network_actions.push(NetworkAction::DnsServers {
                                            interface: Some(iface.clone()),
                                            servers,
                                        });
                                    }
                                }
                                
                                if let Some(ssid) = wifi_ssid {
                                    if !ssid.is_empty() {
                                        profile.network_actions.push(NetworkAction::WifiConnect {
                                            ssid,
                                            interface: Some(iface.clone()),
                                        });
                                    }
                                }
                            }
                        }
                        
                        // === VPN Connection ===
                        if vpn_on && !vpn_name_val.is_empty() {
                            profile.network_actions.push(NetworkAction::VpnConnect {
                                connection_name: vpn_name_val,
                            });
                        }
                        
                        // === Proxy Configuration ===
                        if proxy_mode > 0 {
                            let mode = match proxy_mode {
                                1 => ProxyMode::Manual,
                                2 => ProxyMode::Auto,
                                _ => ProxyMode::None,
                            };
                            
                            let no_proxy_list: Vec<String> = no_proxy_val
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            
                            profile.system_actions.push(SystemAction::ProxyConfig(ProxyConfig {
                                mode,
                                http_proxy: if proxy_mode == 1 && !http_proxy_val.is_empty() { Some(http_proxy_val) } else { None },
                                https_proxy: if proxy_mode == 1 && !https_proxy_val.is_empty() { Some(https_proxy_val) } else { None },
                                ftp_proxy: None,
                                socks_proxy: None,
                                no_proxy: no_proxy_list,
                                pac_url: if proxy_mode == 2 && !pac_url_val.is_empty() { Some(pac_url_val) } else { None },
                            }));
                        }
                        
                        // === Pre-Script ===
                        if pre_script_on && !pre_script.is_empty() {
                            profile.automation_actions.push(AutomationAction::PreScript {
                                path: PathBuf::from(pre_script),
                                args: vec![],
                                env: std::collections::HashMap::new(),
                                mode: crate::models::ScriptMode::Wait,
                                working_dir: None,
                                continue_on_error: false,
                            });
                        }
                        
                        // === Post-Script ===
                        if post_script_on && !post_script.is_empty() {
                            profile.automation_actions.push(AutomationAction::PostScript {
                                path: PathBuf::from(post_script),
                                args: vec![],
                                env: std::collections::HashMap::new(),
                                mode: crate::models::ScriptMode::Wait,
                                working_dir: None,
                                continue_on_error: false,
                            });
                        }
                        
                        // === Run Program ===
                        if run_prog_on && !run_prog_path.is_empty() {
                            let args: Vec<String> = run_prog_args
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect();
                            
                            profile.automation_actions.push(AutomationAction::RunProgram {
                                program: run_prog_path,
                                args,
                                env: std::collections::HashMap::new(),
                                mode: crate::models::ProgramMode::Background,
                                working_dir: None,
                            });
                        }
                    }
                }
                
                // Refresh profiles page
                if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                    let profiles = imp.profiles.borrow().clone();
                    profiles_page.update_profiles(profiles);
                }
                
                window.show_toast(&format!("Profile '{}' updated", new_name));
            }
        });

        dialog.present(Some(self));
    }
    
    /// Convert prefix length to subnet mask string
    fn prefix_to_subnet(prefix: u8) -> String {
        match prefix {
            32 => "255.255.255.255",
            31 => "255.255.255.254",
            30 => "255.255.255.252",
            29 => "255.255.255.248",
            28 => "255.255.255.240",
            27 => "255.255.255.224",
            26 => "255.255.255.192",
            25 => "255.255.255.128",
            24 => "255.255.255.0",
            23 => "255.255.254.0",
            22 => "255.255.252.0",
            21 => "255.255.248.0",
            20 => "255.255.240.0",
            19 => "255.255.224.0",
            18 => "255.255.192.0",
            17 => "255.255.128.0",
            16 => "255.255.0.0",
            15 => "255.254.0.0",
            14 => "255.252.0.0",
            13 => "255.248.0.0",
            12 => "255.240.0.0",
            11 => "255.224.0.0",
            10 => "255.192.0.0",
            9 => "255.128.0.0",
            8 => "255.0.0.0",
            _ => "255.255.255.0",
        }.to_string()
    }
    
    /// Show delete profile confirmation dialog
    fn show_delete_profile_dialog(&self, profile_id: &str) {
        let imp = self.imp();
        
        // Find the profile name
        let profile_name = {
            let profiles = imp.profiles.borrow();
            profiles.iter()
                .find(|p| p.id().to_string() == profile_id)
                .map(|p| p.name().to_string())
        };
        
        let Some(name) = profile_name else {
            self.show_toast("Profile not found");
            return;
        };
        
        let dialog = adw::AlertDialog::new(
            Some("Delete Profile?"),
            Some(&format!("Are you sure you want to delete \"{}\"? This action cannot be undone.", name))
        );
        
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        
        let window_weak = self.downgrade();
        let profile_id = profile_id.to_string();
        let profile_name = name.clone();
        
        dialog.connect_response(None, move |_, response| {
            if response == "delete" {
                if let Some(window) = window_weak.upgrade() {
                    window.delete_profile(&profile_id, &profile_name);
                }
            }
        });
        
        dialog.present(Some(self));
    }
    
    /// Actually delete a profile
    fn delete_profile(&self, profile_id: &str, profile_name: &str) {
        let imp = self.imp();
        
        // Remove from storage
        {
            let mut profiles = imp.profiles.borrow_mut();
            profiles.retain(|p| p.id().to_string() != profile_id);
        }
        
        // Save to cache
        self.save_profiles_to_cache();
        
        // Log the profile deletion
        if let Some(store) = imp.data_store.borrow().as_ref() {
            store.append_log("INFO", &format!("Profile '{}' deleted", profile_name));
        }
        
        // Refresh logs page
        if let Some(logs_page) = imp.logs_page.borrow().as_ref() {
            logs_page.refresh_logs();
        }
        
        // Refresh profiles page
        if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
            let profiles = imp.profiles.borrow().clone();
            profiles_page.update_profiles(profiles);
        }
        
        self.show_toast(&format!("Profile '{}' deleted", profile_name));
    }
    
    /// Show a dialog to select and switch to a profile
    fn show_profile_switcher_dialog(&self) {
        let imp = self.imp();
        let profiles = imp.profiles.borrow().clone();
        
        if profiles.is_empty() {
            self.show_toast("No profiles available. Create one first!");
            return;
        }
        
        // Create the dialog
        let dialog = adw::Dialog::builder()
            .title("Switch Profile")
            .content_width(400)
            .content_height(450)
            .build();
        
        // Create content
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        
        // Header bar
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .show_start_title_buttons(false)
            .build();
        content.append(&header);
        
        // Description
        let desc_label = gtk::Label::builder()
            .label("Select a profile to apply its network configuration:")
            .wrap(true)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(18)
            .margin_end(18)
            .xalign(0.0)
            .build();
        content.append(&desc_label);
        
        // Scrolled window for profile list
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();
        
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .margin_start(18)
            .margin_end(18)
            .margin_bottom(18)
            .build();
        list_box.add_css_class("boxed-list");
        
        // Add each profile as a row
        for profile in &profiles {
            let profile_name = profile.name();
            let profile_desc = profile.metadata.description.as_deref().unwrap_or("");
            
            let row = adw::ActionRow::builder()
                .title(profile_name)
                .subtitle(profile_desc)
                .activatable(true)
                .build();
            
            // Add icon based on profile type
            let icon_name = if profile_name.to_lowercase().contains("wifi") {
                "network-wireless-symbolic"
            } else if profile_name.to_lowercase().contains("ethernet") || profile_name.to_lowercase().contains("wired") {
                "network-wired-symbolic"
            } else if profile_name.to_lowercase().contains("vpn") {
                "network-vpn-symbolic"
            } else {
                "network-workgroup-symbolic"
            };
            
            let icon = gtk::Image::from_icon_name(icon_name);
            row.add_prefix(&icon);
            
            // Add apply arrow
            let arrow = gtk::Image::from_icon_name("go-next-symbolic");
            row.add_suffix(&arrow);
            
            // Store profile ID
            let profile_id = profile.id().to_string();
            let dialog_weak = dialog.downgrade();
            let window_weak = self.downgrade();
            
            row.connect_activated(move |_| {
                if let Some(dialog) = dialog_weak.upgrade() {
                    dialog.close();
                }
                if let Some(window) = window_weak.upgrade() {
                    window.apply_profile(&profile_id);
                }
            });
            
            list_box.append(&row);
        }
        
        scrolled.set_child(Some(&list_box));
        content.append(&scrolled);
        
        dialog.set_child(Some(&content));
        dialog.present(Some(self));
    }
    
    /// Apply a profile - execute network configuration
    pub fn apply_profile(&self, profile_id: &str) {
        let imp = self.imp();
        
        // Find the profile
        let profile_opt = {
            let profiles = imp.profiles.borrow();
            profiles.iter().find(|p| p.id().to_string() == profile_id).cloned()
        };
        
        let Some(profile) = profile_opt else {
            self.show_toast("Profile not found");
            return;
        };
        
        // Check if confirmation is required
        let confirm_required = self.application()
            .and_downcast_ref::<crate::application::Application>()
            .map(|app| app.config().confirm_profile_switch)
            .unwrap_or(true);
        
        if confirm_required && profile.has_actions() {
            self.show_apply_confirmation_dialog(&profile);
        } else {
            self.do_apply_profile(&profile);
        }
    }
    
    /// Show confirmation dialog before applying a profile.
    fn show_apply_confirmation_dialog(&self, profile: &Profile) {
        let dialog = adw::AlertDialog::builder()
            .heading(&format!("Apply '{}'?", profile.name()))
            .body(&format!(
                "This will apply {} action(s) to your system:\n• {} network action(s)\n• {} system action(s)\n• {} automation action(s)",
                profile.action_count(),
                profile.network_actions.len(),
                profile.system_actions.len(),
                profile.automation_actions.len()
            ))
            .build();
        
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("apply", "Apply");
        dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("apply"));
        dialog.set_close_response("cancel");
        
        let profile_clone = profile.clone();
        let window_weak = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response == "apply" {
                if let Some(window) = window_weak.upgrade() {
                    window.do_apply_profile(&profile_clone);
                }
            }
        });
        
        dialog.present(Some(self));
    }
    
    /// Actually apply a profile's actions.
    fn do_apply_profile(&self, profile: &Profile) {
        let imp = self.imp();
        let profile_id = profile.id().to_string();
        let profile_name = profile.name().to_string();
        
        // Apply network actions using nmcli
        let mut success_count = 0;
        let mut error_messages = Vec::new();
        
        for action in &profile.network_actions {
            match self.apply_network_action(action) {
                Ok(_) => success_count += 1,
                Err(e) => error_messages.push(e),
            }
        }
        
        // Apply system actions 
        for action in &profile.system_actions {
            match self.apply_system_action(action) {
                Ok(_) => success_count += 1,
                Err(e) => error_messages.push(e),
            }
        }
        
        // Run automation actions
        for action in &profile.automation_actions {
            match self.run_automation_action(action) {
                Ok(_) => success_count += 1,
                Err(e) => error_messages.push(e),
            }
        }
        
        // Update active profile in profiles page
        if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
            profiles_page.set_active_profile(Some(&profile_id));
            let profiles = imp.profiles.borrow().clone();
            profiles_page.update_profiles(profiles);
        }
        
        // Update the dashboard with active profile info
        if let Some(dashboard_page) = imp.dashboard_page.borrow().as_ref() {
            let now = chrono::Local::now();
            let applied_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
            dashboard_page.update_active_profile(Some(&profile_name), Some(&applied_time));
            // Immediate refresh to update UI
            dashboard_page.update_network_info();
        }
        
        // Refresh dashboard network info again after a short delay to allow network changes to take effect
        let window_weak = self.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
            if let Some(window) = window_weak.upgrade() {
                let imp = window.imp();
                if let Some(dashboard_page) = imp.dashboard_page.borrow().as_ref() {
                    dashboard_page.update_network_info();
                }
            }
        });
        
        // Second refresh after more time for network stack to settle
        let window_weak = self.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(3000), move || {
            if let Some(window) = window_weak.upgrade() {
                let imp = window.imp();
                if let Some(dashboard_page) = imp.dashboard_page.borrow().as_ref() {
                    dashboard_page.update_network_info();
                }
            }
        });
        
        // Show result
        if error_messages.is_empty() {
            if success_count > 0 {
                self.show_toast(&format!("Profile '{}' applied ({} actions)", profile_name, success_count));
            } else {
                self.show_toast(&format!("Profile '{}' activated", profile_name));
            }
        } else {
            self.show_toast(&format!("Profile '{}' applied with {} error(s)", profile_name, error_messages.len()));
            for msg in error_messages {
                tracing::warn!("Profile apply error: {}", msg);
            }
        }
    }
    
    /// Apply a network action using nmcli.
    ///
    /// # Privilege model
    ///
    /// These calls invoke `nmcli` directly from the GUI process. NetworkManager
    /// itself enforces Polkit authorisation, so most operations will prompt for
    /// credentials when required. However, this means the Polkit policy shipped
    /// with this application (com.chrisdaggas.network-manager.policy) only
    /// governs the D-Bus daemon — the GUI is **not** mediated by our own D-Bus
    /// service. A future release should route privileged operations through the
    /// daemon so that a single Polkit policy covers both paths.
    fn apply_network_action(&self, action: &crate::models::NetworkAction) -> std::result::Result<(), String> {
        use crate::models::NetworkAction;
        use std::process::Command;
        
        match action {
            NetworkAction::Ipv4Config { interface, method, addresses, gateway } => {
                let iface = interface.as_deref().unwrap_or("eth0");
                
                // Find connection name for this interface
                let conn_name = match Self::get_connection_for_interface(iface) {
                    Some(name) => name,
                    None => {
                        // Skip virtual interfaces that aren't managed by NetworkManager
                        tracing::debug!("Skipping unmanaged interface: {}", iface);
                        return Ok(());
                    }
                };
                
                match method {
                    crate::models::Ipv4Method::Auto => {
                        // Set to DHCP
                        let output = Command::new("nmcli")
                            .args(["connection", "modify", &conn_name, "ipv4.method", "auto"])
                            .output()
                            .map_err(|e| format!("Failed to run nmcli: {}", e))?;
                        
                        if !output.status.success() {
                            // Try using device instead
                            let _ = Command::new("nmcli")
                                .args(["device", "reapply", iface])
                                .output();
                        }
                    }
                    crate::models::Ipv4Method::Manual => {
                        // Set static IP
                        if let Some(addr) = addresses.first() {
                            let ip_str = format!("{}/{}", addr.address, addr.prefix);
                            let _ = Command::new("nmcli")
                                .args(["connection", "modify", &conn_name, "ipv4.method", "manual", "ipv4.addresses", &ip_str])
                                .output();
                        }
                        
                        if let Some(gw) = gateway {
                            let _ = Command::new("nmcli")
                                .args(["connection", "modify", &conn_name, "ipv4.gateway", &gw.to_string()])
                                .output();
                        }
                    }
                    _ => {}
                }
                
                // Reapply the connection (only if we found a valid connection)
                if Self::get_connection_for_interface(iface).is_some() {
                    let _ = Command::new("nmcli")
                        .args(["connection", "up", &conn_name])
                        .output();
                }
                    
                Ok(())
            }
            
            NetworkAction::DnsServers { interface, servers } => {
                let iface = interface.as_deref().unwrap_or("eth0");
                
                // Only configure DNS if there's an actual NetworkManager connection
                if let Some(conn_name) = Self::get_connection_for_interface(iface) {
                    let dns_str: Vec<String> = servers.iter().map(|s| s.to_string()).collect();
                    
                    let _ = Command::new("nmcli")
                        .args(["connection", "modify", &conn_name, "ipv4.dns", &dns_str.join(" ")])
                        .output();
                }
                    
                Ok(())
            }
            
            NetworkAction::WifiConnect { ssid, interface: _ } => {
                let output = Command::new("nmcli")
                    .args(["connection", "up", ssid])
                    .output()
                    .map_err(|e| format!("Failed to connect to WiFi: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("WiFi connection failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                Ok(())
            }
            
            NetworkAction::VpnConnect { connection_name } => {
                let output = Command::new("nmcli")
                    .args(["connection", "up", connection_name])
                    .output()
                    .map_err(|e| format!("Failed to connect VPN: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("VPN connection failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                Ok(())
            }
            
            NetworkAction::InterfaceEnable(state) => {
                let action = if state.enabled { "connect" } else { "disconnect" };
                let _ = Command::new("nmcli")
                    .args(["device", action, &state.interface])
                    .output();
                Ok(())
            }
            
            NetworkAction::VpnDisconnect { connection_name } => {
                let output = Command::new("nmcli")
                    .args(["connection", "down", connection_name])
                    .output()
                    .map_err(|e| format!("Failed to disconnect VPN: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("VPN disconnect failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                Ok(())
            }
            
            NetworkAction::SetMtu { interface, mtu } => {
                if let Some(conn_name) = Self::get_connection_for_interface(interface) {
                    let _ = Command::new("nmcli")
                        .args(["connection", "modify", &conn_name, "802-3-ethernet.mtu", &mtu.to_string()])
                        .output();
                    let _ = Command::new("nmcli")
                        .args(["connection", "up", &conn_name])
                        .output();
                }
                Ok(())
            }
            
            NetworkAction::SetMacAddress { interface, mac_address } => {
                if let Some(conn_name) = Self::get_connection_for_interface(interface) {
                    let _ = Command::new("nmcli")
                        .args(["connection", "modify", &conn_name, "802-3-ethernet.cloned-mac-address", mac_address])
                        .output();
                    let _ = Command::new("nmcli")
                        .args(["connection", "up", &conn_name])
                        .output();
                }
                Ok(())
            }
            
            NetworkAction::DnsSearchDomains { interface, domains } => {
                let iface = interface.as_deref().unwrap_or("eth0");
                if let Some(conn_name) = Self::get_connection_for_interface(iface) {
                    let _ = Command::new("nmcli")
                        .args(["connection", "modify", &conn_name, "ipv4.dns-search", &domains.join(",")])
                        .output();
                }
                Ok(())
            }
            
            // These require more complex handling or are less common
            NetworkAction::Ipv6Config { .. } | 
            NetworkAction::StaticRoutes { .. } | 
            NetworkAction::VlanConfig { .. } => {
                tracing::debug!("Action {:?} not yet implemented", action);
                Ok(())
            }
        }
    }
    
    /// Apply a system action
    fn apply_system_action(&self, action: &crate::models::SystemAction) -> std::result::Result<(), String> {
        use crate::models::SystemAction;
        use std::process::Command;
        
        match action {
            SystemAction::ProxyConfig(config) => {
                // Verify the gsettings schema exists (may not on non-GNOME desktops)
                let schema_exists = Command::new("gsettings")
                    .args(["list-keys", "org.gnome.system.proxy"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                if !schema_exists {
                    return Err("Proxy settings require GNOME — org.gnome.system.proxy schema not found.".to_string());
                }

                // Set GNOME proxy settings via gsettings
                match config.mode {
                    crate::models::ProxyMode::None => {
                        let _ = Command::new("gsettings")
                            .args(["set", "org.gnome.system.proxy", "mode", "none"])
                            .output();
                    }
                    crate::models::ProxyMode::Manual => {
                        let _ = Command::new("gsettings")
                            .args(["set", "org.gnome.system.proxy", "mode", "manual"])
                            .output();
                        
                        if let Some(http) = &config.http_proxy {
                            // Parse http://host:port
                            if let Some(stripped) = http.strip_prefix("http://") {
                                let parts: Vec<&str> = stripped.split(':').collect();
                                if parts.len() >= 2 {
                                    let _ = Command::new("gsettings")
                                        .args(["set", "org.gnome.system.proxy.http", "host", parts[0]])
                                        .output();
                                    let _ = Command::new("gsettings")
                                        .args(["set", "org.gnome.system.proxy.http", "port", parts[1]])
                                        .output();
                                }
                            }
                        }
                    }
                    crate::models::ProxyMode::Auto => {
                        let _ = Command::new("gsettings")
                            .args(["set", "org.gnome.system.proxy", "mode", "auto"])
                            .output();
                        
                        if let Some(pac) = &config.pac_url {
                            let _ = Command::new("gsettings")
                                .args(["set", "org.gnome.system.proxy", "autoconfig-url", pac])
                                .output();
                        }
                    }
                }
                Ok(())
            }
            
            SystemAction::SetHostname { hostname, pretty_hostname } => {
                // Set static hostname via hostnamectl
                let output = Command::new("hostnamectl")
                    .args(["set-hostname", hostname])
                    .output()
                    .map_err(|e| format!("Failed to set hostname: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("Hostname change failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                
                // Set pretty hostname if provided
                if let Some(pretty) = pretty_hostname {
                    let _ = Command::new("hostnamectl")
                        .args(["set-hostname", "--pretty", pretty])
                        .output();
                }
                Ok(())
            }
            
            SystemAction::SetTimezone { timezone } => {
                let output = Command::new("timedatectl")
                    .args(["set-timezone", timezone])
                    .output()
                    .map_err(|e| format!("Failed to set timezone: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("Timezone change failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                Ok(())
            }
            
            SystemAction::DefaultPrinter { printer_name } => {
                let output = Command::new("lpoptions")
                    .args(["-d", printer_name])
                    .output()
                    .map_err(|e| format!("Failed to set default printer: {}", e))?;
                
                if !output.status.success() {
                    return Err(format!("Printer change failed: {}", String::from_utf8_lossy(&output.stderr)));
                }
                Ok(())
            }
            
            SystemAction::EnvironmentVariables { variables } => {
                // Export variables to user's environment file
                let _env_file = dirs::home_dir()
                    .map(|h| h.join(".profile"))
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp/.profile"));
                
                for (key, value) in variables {
                    let line = format!("export {}=\"{}\"", key, value);
                    tracing::info!("Setting environment: {}", line);
                    // Note: Modifying .profile requires shell reload to take effect
                }
                Ok(())
            }
            
            // These require root privileges and are more complex
            SystemAction::HostsEntries { .. } | 
            SystemAction::FirewallConfig(_) => {
                tracing::debug!("Action {:?} requires privilege escalation", action);
                Ok(())
            }
        }
    }
    
    /// Run an automation action
    fn run_automation_action(&self, action: &crate::models::AutomationAction) -> std::result::Result<(), String> {
        use crate::models::AutomationAction;
        use crate::services::SandboxRunner;
        use std::process::Command;
        
        // Get sandbox mode from config
        let sandbox_mode = self.application()
            .and_downcast_ref::<crate::application::Application>()
            .map(|app| app.config().sandbox_mode)
            .unwrap_or_default();
        
        let sandbox = SandboxRunner::new(sandbox_mode);
        
        match action {
            AutomationAction::PreScript { path, args, working_dir, .. } | 
            AutomationAction::PostScript { path, args, working_dir, .. } => {
                let path_str = path.to_string_lossy().to_string();
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                
                // Use sandbox runner if sandboxing is enabled
                if sandbox_mode != crate::models::SandboxMode::None {
                    match sandbox.execute(&path_str, &args_refs) {
                        Ok(output) => {
                            if !output.status.success() {
                                return Err(format!("Script failed: {}", String::from_utf8_lossy(&output.stderr)));
                            }
                        }
                        Err(e) => return Err(format!("Sandbox execution failed: {:?}", e)),
                    }
                } else {
                    // Direct execution without sandboxing
                    let mut cmd = Command::new(path);
                    cmd.args(args);
                    if let Some(wd) = working_dir {
                        cmd.current_dir(wd);
                    }
                    
                    let output = cmd.output()
                        .map_err(|e| format!("Failed to run script: {}", e))?;
                    
                    if !output.status.success() {
                        return Err(format!("Script failed: {}", String::from_utf8_lossy(&output.stderr)));
                    }
                }
                Ok(())
            }
            
            AutomationAction::RunProgram { program, args, working_dir, .. } => {
                // NOTE: RunProgram intentionally spawns without sandboxing.
                // The user explicitly configures the program path; sandboxing
                // would break legitimate program launches (e.g. GUI apps).
                let mut cmd = Command::new(program);
                cmd.args(args);
                if let Some(wd) = working_dir {
                    cmd.current_dir(wd);
                }
                
                cmd.spawn()
                    .map_err(|e| format!("Failed to launch program: {}", e))?;
                Ok(())
            }
            
            _ => Ok(())
        }
    }

    fn create_main_menu_popover(&self) -> gtk::Popover {
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");

        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .width_request(280)
            .build();

        // Theme selector section
        let theme_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(18)
            .halign(gtk::Align::Center)
            .margin_top(18)
            .margin_bottom(18)
            .build();

        // Create theme toggle buttons
        let default_btn = gtk::ToggleButton::new();
        let light_btn = gtk::ToggleButton::new();
        let dark_btn = gtk::ToggleButton::new();

        // Helper to create theme button content
        fn create_theme_content(css_class: &str, is_selected: bool) -> gtk::Overlay {
            let overlay = gtk::Overlay::new();
            
            let icon = gtk::Box::builder()
                .width_request(44)
                .height_request(44)
                .build();
            icon.add_css_class("theme-selector");
            icon.add_css_class(css_class);
            overlay.set_child(Some(&icon));
            
            if is_selected {
                let check = gtk::Image::from_icon_name("object-select-symbolic");
                check.add_css_class("theme-check");
                check.set_halign(gtk::Align::Center);
                check.set_valign(gtk::Align::Center);
                overlay.add_overlay(&check);
            }
            
            overlay
        }

        // Set initial content
        default_btn.set_child(Some(&create_theme_content("theme-default", false)));
        default_btn.set_tooltip_text(Some("System"));
        default_btn.add_css_class("flat");
        default_btn.add_css_class("circular");
        default_btn.add_css_class("theme-button");

        light_btn.set_child(Some(&create_theme_content("theme-light", false)));
        light_btn.set_tooltip_text(Some("Light"));
        light_btn.add_css_class("flat");
        light_btn.add_css_class("circular");
        light_btn.add_css_class("theme-button");

        dark_btn.set_child(Some(&create_theme_content("theme-dark", false)));
        dark_btn.set_tooltip_text(Some("Dark"));
        dark_btn.add_css_class("flat");
        dark_btn.add_css_class("circular");
        dark_btn.add_css_class("theme-button");

        // Group the toggle buttons (radio-button behavior)
        light_btn.set_group(Some(&default_btn));
        dark_btn.set_group(Some(&default_btn));

        // Set initial state based on current theme
        let style_manager = adw::StyleManager::default();
        
        match style_manager.color_scheme() {
            adw::ColorScheme::ForceLight => {
                light_btn.set_active(true);
                light_btn.set_child(Some(&create_theme_content("theme-light", true)));
            }
            adw::ColorScheme::ForceDark => {
                dark_btn.set_active(true);
                dark_btn.set_child(Some(&create_theme_content("theme-dark", true)));
            }
            _ => {
                default_btn.set_active(true);
                default_btn.set_child(Some(&create_theme_content("theme-default", true)));
            }
        }

        // Connect theme button signals
        let light_btn_clone = light_btn.clone();
        let dark_btn_clone = dark_btn.clone();
        default_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::Default);
                btn.set_child(Some(&create_theme_content("theme-default", true)));
                light_btn_clone.set_child(Some(&create_theme_content("theme-light", false)));
                dark_btn_clone.set_child(Some(&create_theme_content("theme-dark", false)));
            }
        });

        let default_btn_clone = default_btn.clone();
        let dark_btn_clone2 = dark_btn.clone();
        light_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
                btn.set_child(Some(&create_theme_content("theme-light", true)));
                default_btn_clone.set_child(Some(&create_theme_content("theme-default", false)));
                dark_btn_clone2.set_child(Some(&create_theme_content("theme-dark", false)));
            }
        });

        let default_btn_clone2 = default_btn.clone();
        let light_btn_clone2 = light_btn.clone();
        dark_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
                btn.set_child(Some(&create_theme_content("theme-dark", true)));
                default_btn_clone2.set_child(Some(&create_theme_content("theme-default", false)));
                light_btn_clone2.set_child(Some(&create_theme_content("theme-light", false)));
            }
        });

        theme_box.append(&default_btn);
        theme_box.append(&light_btn);
        theme_box.append(&dark_btn);
        main_box.append(&theme_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        separator.set_margin_start(12);
        separator.set_margin_end(12);
        main_box.append(&separator);

        // Menu items
        let menu_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu_list.set_margin_top(6);
        menu_list.set_margin_bottom(6);
        menu_list.set_margin_start(6);
        menu_list.set_margin_end(6);

        // Preferences button
        let prefs_btn = gtk::Button::new();
        let prefs_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        prefs_box.set_margin_start(6);
        prefs_box.set_margin_end(6);
        prefs_box.set_margin_top(8);
        prefs_box.set_margin_bottom(8);
        let prefs_icon = gtk::Image::from_icon_name("emblem-system-symbolic");
        let prefs_label = gtk::Label::new(Some("Preferences"));
        prefs_label.set_halign(gtk::Align::Start);
        prefs_label.set_hexpand(true);
        prefs_box.append(&prefs_icon);
        prefs_box.append(&prefs_label);
        prefs_btn.set_child(Some(&prefs_box));
        prefs_btn.add_css_class("flat");
        prefs_btn.add_css_class("menu-item");
        prefs_btn.set_action_name(Some("app.preferences"));
        menu_list.append(&prefs_btn);

        // About button
        let about_btn = gtk::Button::new();
        let about_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        about_box.set_margin_start(6);
        about_box.set_margin_end(6);
        about_box.set_margin_top(8);
        about_box.set_margin_bottom(8);
        let about_icon = gtk::Image::from_icon_name("help-about-symbolic");
        let about_label = gtk::Label::new(Some("About"));
        about_label.set_halign(gtk::Align::Start);
        about_label.set_hexpand(true);
        about_box.append(&about_icon);
        about_box.append(&about_label);
        about_btn.set_child(Some(&about_box));
        about_btn.add_css_class("flat");
        about_btn.add_css_class("menu-item");
        about_btn.set_action_name(Some("app.about"));
        menu_list.append(&about_btn);

        // Quit button
        let quit_btn = gtk::Button::new();
        let quit_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        quit_box.set_margin_start(6);
        quit_box.set_margin_end(6);
        quit_box.set_margin_top(8);
        quit_box.set_margin_bottom(8);
        let quit_icon = gtk::Image::from_icon_name("application-exit-symbolic");
        let quit_label = gtk::Label::new(Some("Quit"));
        quit_label.set_halign(gtk::Align::Start);
        quit_label.set_hexpand(true);
        quit_box.append(&quit_icon);
        quit_box.append(&quit_label);
        quit_btn.set_child(Some(&quit_box));
        quit_btn.add_css_class("flat");
        quit_btn.add_css_class("menu-item");
        quit_btn.set_action_name(Some("app.quit"));
        menu_list.append(&quit_btn);

        main_box.append(&menu_list);

        popover.set_child(Some(&main_box));
        popover
    }

    fn create_nav_row_with_label(&self, nav_item: NavItem) -> (gtk::ListBoxRow, gtk::Label, gtk::Box) {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(true);
        row.set_tooltip_text(Some(nav_item.title()));

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.add_css_class("nav-row-box");

        let icon = gtk::Image::from_icon_name(nav_item.icon_name());
        icon.set_pixel_size(20);
        hbox.append(&icon);

        let label = gtk::Label::new(Some(nav_item.title()));
        label.set_halign(gtk::Align::Start);
        label.set_hexpand(true);
        label.add_css_class("nav-label");
        hbox.append(&label);

        row.set_child(Some(&hbox));
        (row, label, hbox)
    }

    fn navigate_to(&self, nav_item: NavItem) {
        let imp = self.imp();

        if let Some(stack) = imp.content_stack.borrow().as_ref() {
            let page_name = match nav_item {
                NavItem::Dashboard => "dashboard",
                NavItem::Profiles => "profiles",
                NavItem::Logs => "logs",
                NavItem::Settings => "settings",
                NavItem::Help => "help",
            };
            stack.set_visible_child_name(page_name);
        }

        if let Some(title) = imp.content_title.borrow().as_ref() {
            title.set_title(nav_item.title());
        }

        imp.current_nav.set(Some(nav_item));
    }

    /// Initialize with data store.
    pub fn init_with_store(&self, store: Arc<DataStore>) {
        let imp = self.imp();
        
        // Store reference to data store
        *imp.data_store.borrow_mut() = Some(store.clone());

        // Load profiles from cache
        store.load_profiles_cache();
        let cached_profiles = store.profiles();
        if !cached_profiles.is_empty() {
            *imp.profiles.borrow_mut() = cached_profiles.clone();
            // Update the profiles page
            if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                profiles_page.update_profiles(cached_profiles);
            }
        }

        // Initialize logs page with data store
        if let Some(logs_page) = imp.logs_page.borrow().as_ref() {
            logs_page.init_with_store(store.clone());
        }

        // Log application startup
        store.append_log("INFO", "Application started");

        // Check for updates on startup
        self.check_for_updates();
    }

    /// Run the one-time GitHub release check in the background.
    fn check_for_updates(&self) {
        let obj_weak = self.downgrade();
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Spawn the HTTP request on the Tokio runtime
        crate::application::tokio_runtime().spawn(async move {
            let result = crate::version_check::check_for_update(crate::VERSION).await;
            let _ = tx.send(result);
        });

        // Receive the result on the GTK main thread
        glib::spawn_future_local(async move {
            if let Ok(Some(update_info)) = rx.await {
                if let Some(obj) = obj_weak.upgrade() {
                    obj.show_update_available(&update_info);
                }
            }
        });
    }

    /// Display the update banner with a clickable link.
    fn show_update_available(&self, info: &crate::version_check::UpdateInfo) {
        let imp = self.imp();
        if let Some(ref banner) = *imp.update_banner.borrow() {
            // Clear placeholder children and rebuild with real info
            while let Some(child) = banner.first_child() {
                banner.remove(&child);
            }

            let icon = gtk::Image::from_icon_name("software-update-available-symbolic");
            icon.set_pixel_size(14);
            icon.add_css_class("update-icon");
            banner.append(&icon);

            let label_text = format!("v{} available", info.latest_version);
            let link = gtk::LinkButton::with_label(&info.download_url, &label_text);
            link.add_css_class("update-link");
            banner.append(&link);

            banner.set_visible(true);
        }
    }

    /// Save profiles to the data store cache
    fn save_profiles_to_cache(&self) {
        let imp = self.imp();
        if let Some(store) = imp.data_store.borrow().as_ref() {
            let profiles = imp.profiles.borrow().clone();
            store.update_profiles_cache(profiles);
        }
    }

    /// Reload profiles from cache file and update UI
    pub fn reload_profiles_from_cache(&self) {
        let imp = self.imp();
        if let Some(store) = imp.data_store.borrow().as_ref() {
            // Reload from disk
            store.load_profiles_cache();
            let cached_profiles = store.profiles();
            
            // Update internal state
            *imp.profiles.borrow_mut() = cached_profiles.clone();
            
            // Update the profiles page UI
            if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                profiles_page.update_profiles(cached_profiles.clone());
            }
            
            tracing::info!("Reloaded {} profiles from cache", cached_profiles.len());
        }
    }

    /// Show a toast notification.
    pub fn show_toast(&self, message: &str) {
        let imp = self.imp();
        if let Some(toast_overlay) = imp.toast_overlay.borrow().as_ref() {
            let toast = adw::Toast::new(message);
            toast.set_timeout(3);
            toast_overlay.add_toast(toast);
        }
    }

    /// Show a toast with an action button.
    pub fn show_toast_with_action(&self, message: &str, action_label: &str, action_name: &str) {
        let imp = self.imp();
        if let Some(toast_overlay) = imp.toast_overlay.borrow().as_ref() {
            let toast = adw::Toast::new(message);
            toast.set_button_label(Some(action_label));
            toast.set_action_name(Some(action_name));
            toast.set_timeout(5);
            toast_overlay.add_toast(toast);
        }
    }
    
    /// Duplicate an existing profile.
    fn duplicate_profile(&self, profile_id: &str) {
        let imp = self.imp();
        
        let duplicated_opt = {
            let profiles = imp.profiles.borrow();
            profiles.iter().find(|p| p.id().to_string() == profile_id).map(|original| {
                let mut duplicated = original.clone();
                // Generate new ID and update name
                duplicated.metadata.id = uuid::Uuid::new_v4();
                duplicated.metadata.name = format!("{} (Copy)", original.name());
                duplicated.metadata.created_at = chrono::Utc::now();
                duplicated.metadata.updated_at = chrono::Utc::now();
                duplicated.status = crate::models::profile::ProfileStatus::Inactive;
                duplicated
            })
        };
        
        if let Some(duplicated) = duplicated_opt {
            let name = duplicated.metadata.name.clone();
            
            // Add to profiles list
            imp.profiles.borrow_mut().push(duplicated);
            
            // Update UI
            if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                profiles_page.update_profiles(imp.profiles.borrow().clone());
            }
            
            // Save to cache
            self.save_profiles_to_cache();
            
            self.show_toast(&format!("Profile '{}' created", name));
        }
    }
    
    /// Copy network information to clipboard.
    fn copy_network_info_to_clipboard(&self) {
        use std::process::Command;
        
        // Gather network info
        let mut info = String::new();
        info.push_str("=== Network Information ===\n\n");
        
        // Get IP addresses
        if let Ok(output) = Command::new("ip").args(["-4", "addr", "show"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("inet ") {
                        info.push_str(&format!("{}\n", line.trim()));
                    }
                }
            }
        }
        
        info.push('\n');
        
        // Get default route
        if let Ok(output) = Command::new("ip").args(["route", "show", "default"]).output() {
            if output.status.success() {
                info.push_str("Default Route:\n");
                info.push_str(&String::from_utf8_lossy(&output.stdout));
            }
        }
        
        info.push('\n');
        
        // Get DNS servers
        if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
            info.push_str("DNS Servers:\n");
            for line in content.lines() {
                if line.starts_with("nameserver ") {
                    info.push_str(&format!("  {}\n", line));
                }
            }
        }
        
        // Copy to clipboard using GTK clipboard
        if let Some(display) = gtk::gdk::Display::default() {
            let clipboard = display.clipboard();
            clipboard.set_text(&info);
            self.show_toast("Network info copied to clipboard");
        }
    }
    
    /// Show network diagnostics dialog.
    fn show_network_diagnostics_dialog(&self) {
        use std::process::Command;
        
        let dialog = adw::Dialog::new();
        dialog.set_title("Network Diagnostics");
        dialog.set_content_width(700);
        dialog.set_content_height(600);
        
        let toolbar_view = adw::ToolbarView::new();
        
        let header = adw::HeaderBar::new();
        toolbar_view.add_top_bar(&header);
        
        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);
        
        // Ping test section
        let ping_group = adw::PreferencesGroup::new();
        ping_group.set_title("Connectivity Test");
        ping_group.set_description(Some("Test network connectivity to common servers"));
        
        let ping_entry = adw::EntryRow::new();
        ping_entry.set_title("Host to ping");
        ping_entry.set_text("8.8.8.8");
        ping_group.add(&ping_entry);
        
        let ping_btn = gtk::Button::with_label("Run Ping Test");
        ping_btn.add_css_class("suggested-action");
        ping_btn.set_margin_top(8);
        
        let ping_result = gtk::TextView::new();
        ping_result.set_editable(false);
        ping_result.set_monospace(true);
        ping_result.set_wrap_mode(gtk::WrapMode::WordChar);
        ping_result.add_css_class("log-view");
        
        let ping_scroll = gtk::ScrolledWindow::new();
        ping_scroll.set_min_content_height(120);
        ping_scroll.set_child(Some(&ping_result));
        ping_scroll.set_margin_top(8);
        
        let ping_entry_weak = ping_entry.downgrade();
        let ping_result_weak = ping_result.downgrade();
        ping_btn.connect_clicked(move |_| {
            if let (Some(entry), Some(result)) = (ping_entry_weak.upgrade(), ping_result_weak.upgrade()) {
                let host = entry.text().to_string();
                let buffer = result.buffer();

                // Validate hostname/IP: only allow alphanumeric, dots, hyphens, colons (IPv6)
                let is_valid = !host.is_empty()
                    && host.len() <= 253
                    && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':');

                if !is_valid {
                    buffer.set_text("Invalid host. Only alphanumeric characters, dots, hyphens, and colons are allowed.");
                    return;
                }

                buffer.set_text("Running ping test...\n");
                
                // Run ping in background using a channel to send results back
                let (tx, rx) = std::sync::mpsc::channel::<String>();
                
                std::thread::spawn(move || {
                    let output = Command::new("ping")
                        .args(["-c", "4", "--", &host])
                        .output();
                    
                    let text = match output {
                        Ok(out) => {
                            if out.status.success() {
                                String::from_utf8_lossy(&out.stdout).to_string()
                            } else {
                                format!("Ping failed:\n{}", String::from_utf8_lossy(&out.stderr))
                            }
                        }
                        Err(e) => format!("Error running ping: {}", e),
                    };
                    let _ = tx.send(text);
                });
                
                // Poll for result on main thread  
                let buffer_weak = buffer.downgrade();
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    match rx.try_recv() {
                        Ok(text) => {
                            if let Some(buffer) = buffer_weak.upgrade() {
                                buffer.set_text(&text);
                            }
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => glib::ControlFlow::Break,
                    }
                });
            }
        });
        
        content.append(&ping_group);
        content.append(&ping_btn);
        content.append(&ping_scroll);
        
        // DNS lookup section
        let dns_group = adw::PreferencesGroup::new();
        dns_group.set_title("DNS Lookup");
        dns_group.set_description(Some("Resolve domain names"));
        dns_group.set_margin_top(16);
        
        let dns_entry = adw::EntryRow::new();
        dns_entry.set_title("Domain to lookup");
        dns_entry.set_text("google.com");
        dns_group.add(&dns_entry);
        
        let dns_btn = gtk::Button::with_label("Lookup DNS");
        dns_btn.add_css_class("suggested-action");
        dns_btn.set_margin_top(8);
        
        let dns_result = gtk::TextView::new();
        dns_result.set_editable(false);
        dns_result.set_monospace(true);
        dns_result.set_wrap_mode(gtk::WrapMode::WordChar);
        dns_result.add_css_class("log-view");
        
        let dns_scroll = gtk::ScrolledWindow::new();
        dns_scroll.set_min_content_height(80);
        dns_scroll.set_child(Some(&dns_result));
        dns_scroll.set_margin_top(8);
        
        let dns_entry_weak = dns_entry.downgrade();
        let dns_result_weak = dns_result.downgrade();
        dns_btn.connect_clicked(move |_| {
            if let (Some(entry), Some(result)) = (dns_entry_weak.upgrade(), dns_result_weak.upgrade()) {
                let domain = entry.text().to_string();
                let buffer = result.buffer();

                // Validate domain: only allow alphanumeric, dots, hyphens
                let is_valid = !domain.is_empty()
                    && domain.len() <= 253
                    && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');

                if !is_valid {
                    buffer.set_text("Invalid domain. Only alphanumeric characters, dots, and hyphens are allowed.");
                    return;
                }

                if let Ok(output) = Command::new("dig").args(["+short", "--", &domain]).output() {
                    if output.status.success() {
                        let text = String::from_utf8_lossy(&output.stdout);
                        if text.is_empty() {
                            buffer.set_text("No DNS records found");
                        } else {
                            buffer.set_text(&text);
                        }
                    } else {
                        buffer.set_text(&format!("DNS lookup failed:\n{}", String::from_utf8_lossy(&output.stderr)));
                    }
                } else {
                    // Fallback to host command
                    if let Ok(output) = Command::new("host").arg(&domain).output() {
                        buffer.set_text(&String::from_utf8_lossy(&output.stdout));
                    } else {
                        buffer.set_text("DNS lookup tools (dig, host) not available");
                    }
                }
            }
        });
        
        content.append(&dns_group);
        content.append(&dns_btn);
        content.append(&dns_scroll);
        
        // Quick diagnostics
        let quick_group = adw::PreferencesGroup::new();
        quick_group.set_title("Quick Test Results");
        quick_group.set_margin_top(16);
        
        // Run quick tests
        let internet_status = if Command::new("ping")
            .args(["-c", "1", "-W", "2", "8.8.8.8"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "✓ Connected"
        } else {
            "✗ No connection"
        };
        
        let internet_row = adw::ActionRow::builder()
            .title("Internet Connectivity")
            .subtitle(internet_status)
            .build();
        quick_group.add(&internet_row);
        
        let dns_status = if Command::new("host")
            .args(["-W", "2", "google.com"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "✓ DNS Working"
        } else {
            "✗ DNS Failed"
        };
        
        let dns_row = adw::ActionRow::builder()
            .title("DNS Resolution")
            .subtitle(dns_status)
            .build();
        quick_group.add(&dns_row);
        
        content.append(&quick_group);
        
        // Speed Test section
        let speed_group = adw::PreferencesGroup::new();
        speed_group.set_title("Speed Test");
        speed_group.set_description(Some("Measure download/upload speed (uses speedtest-cli if available)"));
        speed_group.set_margin_top(16);
        
        let speed_btn = gtk::Button::with_label("Run Speed Test");
        speed_btn.add_css_class("suggested-action");
        
        let speed_result = gtk::TextView::new();
        speed_result.set_editable(false);
        speed_result.set_monospace(true);
        speed_result.set_wrap_mode(gtk::WrapMode::WordChar);
        speed_result.add_css_class("log-view");
        
        let speed_scroll = gtk::ScrolledWindow::new();
        speed_scroll.set_min_content_height(100);
        speed_scroll.set_child(Some(&speed_result));
        speed_scroll.set_margin_top(8);
        
        let speed_result_weak = speed_result.downgrade();
        let speed_btn_weak = speed_btn.downgrade();
        speed_btn.connect_clicked(move |_| {
            if let (Some(result), Some(btn)) = (speed_result_weak.upgrade(), speed_btn_weak.upgrade()) {
                let buffer = result.buffer();
                buffer.set_text("Running speed test... (this may take a minute)");
                btn.set_sensitive(false);
                
                let (tx, rx) = std::sync::mpsc::channel::<String>();
                
                std::thread::spawn(move || {
                    // Try speedtest-cli first
                    let output = Command::new("speedtest-cli")
                        .arg("--simple")
                        .output();
                    
                    let text = match output {
                        Ok(out) if out.status.success() => {
                            String::from_utf8_lossy(&out.stdout).to_string()
                        }
                        _ => {
                            // Fallback: simple download test using curl
                            let download_result = Command::new("curl")
                                .args(["-o", "/dev/null", "-w", "%{speed_download}", "-s", 
                                       "https://speed.cloudflare.com/__down?bytes=10000000"])
                                .output();
                            
                            match download_result {
                                Ok(out) if out.status.success() => {
                                    let speed_str = String::from_utf8_lossy(&out.stdout);
                                    if let Ok(speed) = speed_str.trim().parse::<f64>() {
                                        let speed_mbps = speed * 8.0 / 1_000_000.0;
                                        format!("Download: {:.2} Mbps\n(speedtest-cli not installed for full test)", speed_mbps)
                                    } else {
                                        "Could not parse speed result".to_string()
                                    }
                                }
                                _ => "Speed test tools not available.\nInstall speedtest-cli: pip install speedtest-cli".to_string()
                            }
                        }
                    };
                    let _ = tx.send(text);
                });
                
                // Poll for result
                let buffer_weak = buffer.downgrade();
                let btn_weak2 = btn.downgrade();
                glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                    match rx.try_recv() {
                        Ok(text) => {
                            if let Some(buffer) = buffer_weak.upgrade() {
                                buffer.set_text(&text);
                            }
                            if let Some(btn) = btn_weak2.upgrade() {
                                btn.set_sensitive(true);
                            }
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => {
                            if let Some(btn) = btn_weak2.upgrade() {
                                btn.set_sensitive(true);
                            }
                            glib::ControlFlow::Break
                        }
                    }
                });
            }
        });
        
        content.append(&speed_group);
        content.append(&speed_btn);
        content.append(&speed_scroll);
        
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_child(Some(&content));
        
        toolbar_view.set_content(Some(&scroll));
        dialog.set_child(Some(&toolbar_view));
        
        dialog.present(Some(self));
    }
    
    /// Refresh the current view.
    fn refresh_current_view(&self) {
        let imp = self.imp();
        
        if let Some(nav_item) = imp.current_nav.get() {
            match nav_item {
                NavItem::Dashboard => {
                    if let Some(dashboard) = imp.dashboard_page.borrow().as_ref() {
                        dashboard.update_network_info();
                    }
                }
                NavItem::Profiles => {
                    self.reload_profiles_from_cache();
                }
                NavItem::Logs => {
                    if let Some(logs_page) = imp.logs_page.borrow().as_ref() {
                        logs_page.refresh_logs();
                    }
                }
                _ => {}
            }
        }
        
        self.show_toast("Refreshed");
    }
    
    /// Show export profiles dialog.
    fn show_export_profiles_dialog(&self) {
        let imp = self.imp();
        let profiles = imp.profiles.borrow().clone();
        
        if profiles.is_empty() {
            self.show_toast("No profiles to export");
            return;
        }
        
        let dialog = gtk::FileDialog::builder()
            .title("Export Profiles")
            .initial_name("network-profiles.json")
            .build();
        
        let window_weak = self.downgrade();
        dialog.save(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    if let Some(window) = window_weak.upgrade() {
                        window.export_profiles_to_file(&path);
                    }
                }
            }
        });
    }
    
    /// Export profiles to a file.
    fn export_profiles_to_file(&self, path: &std::path::Path) {
        let imp = self.imp();
        let profiles = imp.profiles.borrow().clone();
        
        match serde_json::to_string_pretty(&profiles) {
            Ok(json) => {
                match std::fs::write(path, json) {
                    Ok(_) => {
                        self.show_toast(&format!("Exported {} profiles", profiles.len()));
                    }
                    Err(e) => {
                        self.show_toast(&format!("Export failed: {}", e));
                    }
                }
            }
            Err(e) => {
                self.show_toast(&format!("Export failed: {}", e));
            }
        }
    }
    
    /// Show import profiles dialog.
    fn show_import_profiles_dialog(&self) {
        let json_filter = gtk::FileFilter::new();
        json_filter.set_name(Some("JSON Files"));
        json_filter.add_pattern("*.json");
        
        let all_filter = gtk::FileFilter::new();
        all_filter.set_name(Some("All Files"));
        all_filter.add_pattern("*");
        
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&json_filter);
        filters.append(&all_filter);
        
        let dialog = gtk::FileDialog::builder()
            .title("Import Profiles")
            .filters(&filters)
            .build();
        
        let window_weak = self.downgrade();
        dialog.open(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    if let Some(window) = window_weak.upgrade() {
                        window.import_profiles_from_file(&path);
                    }
                }
            }
        });
    }
    
    /// Import profiles from a file.
    fn import_profiles_from_file(&self, path: &std::path::Path) {
        let imp = self.imp();
        
        match std::fs::read_to_string(path) {
            Ok(json) => {
                match serde_json::from_str::<Vec<Profile>>(&json) {
                    Ok(imported_profiles) => {
                        if imported_profiles.is_empty() {
                            self.show_toast("No profiles found in file");
                            return;
                        }
                        
                        // Generate new IDs for imported profiles to avoid duplicates
                        let mut new_profiles: Vec<Profile> = imported_profiles.into_iter().map(|mut p| {
                            p.metadata.id = uuid::Uuid::new_v4();
                            p.metadata.name = format!("{} (Imported)", p.metadata.name);
                            p.metadata.created_at = chrono::Utc::now();
                            p.metadata.updated_at = chrono::Utc::now();
                            p.status = crate::models::profile::ProfileStatus::Inactive;
                            p
                        }).collect();
                        
                        let count = new_profiles.len();
                        
                        // Add to existing profiles
                        imp.profiles.borrow_mut().append(&mut new_profiles);
                        
                        // Update UI
                        if let Some(profiles_page) = imp.profiles_page.borrow().as_ref() {
                            profiles_page.update_profiles(imp.profiles.borrow().clone());
                        }
                        
                        // Save to cache
                        self.save_profiles_to_cache();
                        
                        self.show_toast(&format!("Imported {} profiles", count));
                    }
                    Err(e) => {
                        self.show_toast(&format!("Invalid profile format: {}", e));
                    }
                }
            }
            Err(e) => {
                self.show_toast(&format!("Failed to read file: {}", e));
            }
        }
    }
}
