%global debug_package %{nil}

Name:           network-manager
Version:        0.1.0
Release:        1%{?dist}
Summary:        Network and system profile manager for Linux

License:        MIT
URL:            https://github.com/christosdaggas/network-manager
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.75
BuildRequires:  gtk4-devel >= 4.14
BuildRequires:  libadwaita-devel >= 1.5
BuildRequires:  glib2-devel >= 2.76
BuildRequires:  pkgconfig
BuildRequires:  gettext-devel
BuildRequires:  systemd-rpm-macros

Requires:       gtk4 >= 4.14
Requires:       libadwaita >= 1.5
Requires:       NetworkManager
Requires:       polkit
Recommends:     %{name}-daemon

%description
CD Network Manager is a Linux-native system and network profile
manager inspired by NetSetMan Pro (Windows). It allows you to create,
manage, and quickly switch between comprehensive network and system
configuration profiles.

Features include:
- Network configuration (IPv4/IPv6, DNS, routes, WiFi)
- VPN integration via NetworkManager
- System settings (hostname, hosts, proxy, firewall)
- Automation with pre/post scripts
- Rule-based automatic profile switching
- Polkit integration for security

%package daemon
Summary:        Privileged daemon for CD Network Manager
Requires:       NetworkManager
Requires:       polkit
Requires:       systemd
%{?systemd_requires}

%description daemon
This package contains the privileged D-Bus daemon that executes
profile changes requiring elevated permissions.

The daemon runs as a systemd service and communicates with the
GUI application via D-Bus.

%package cli
Summary:        Command-line interface for CD Network Manager
Requires:       %{name}-daemon

%description cli
This package provides the nmctl command-line tool for controlling
CD Network Manager from the terminal or scripts.

%prep
%autosetup

%build
cargo build --release --workspace

%install
# Install binaries
install -Dm755 target/release/network-manager %{buildroot}%{_bindir}/network-manager
install -Dm755 target/release/network-managerd %{buildroot}%{_bindir}/network-managerd
install -Dm755 target/release/nmctl %{buildroot}%{_bindir}/nmctl

# Install desktop file
install -Dm644 data/com.chrisdaggas.network-manager.desktop \
    %{buildroot}%{_datadir}/applications/com.chrisdaggas.network-manager.desktop

# Install metainfo
install -Dm644 data/com.chrisdaggas.network-manager.metainfo.xml \
    %{buildroot}%{_datadir}/metainfo/com.chrisdaggas.network-manager.metainfo.xml

# Install icons
install -Dm644 data/icons/hicolor/scalable/apps/com.chrisdaggas.network-manager.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/com.chrisdaggas.network-manager.svg
install -Dm644 data/icons/hicolor/symbolic/apps/com.chrisdaggas.network-manager-symbolic.svg \
    %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps/com.chrisdaggas.network-manager-symbolic.svg

# Install D-Bus configuration
install -Dm644 data/dbus-1/com.chrisdaggas.NetworkManagerd.conf \
    %{buildroot}%{_datadir}/dbus-1/system.d/com.chrisdaggas.NetworkManagerd.conf
install -Dm644 data/dbus-1/com.chrisdaggas.NetworkManagerd.service \
    %{buildroot}%{_datadir}/dbus-1/system-services/com.chrisdaggas.NetworkManagerd.service

# Install Polkit policy
install -Dm644 data/polkit/com.chrisdaggas.network-manager.policy \
    %{buildroot}%{_datadir}/polkit-1/actions/com.chrisdaggas.network-manager.policy

# Install systemd service
install -Dm644 data/systemd/network-managerd.service \
    %{buildroot}%{_unitdir}/network-managerd.service

%check
cargo test --release --workspace

%post daemon
%systemd_post network-managerd.service

%preun daemon
%systemd_preun network-managerd.service

%postun daemon
%systemd_postun_with_restart network-managerd.service

%files
%license LICENSE
%doc README.md
%{_bindir}/network-manager
%{_datadir}/applications/com.chrisdaggas.network-manager.desktop
%{_datadir}/metainfo/com.chrisdaggas.network-manager.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/com.chrisdaggas.network-manager.svg
%{_datadir}/icons/hicolor/symbolic/apps/com.chrisdaggas.network-manager-symbolic.svg

%files daemon
%license LICENSE
%{_bindir}/network-managerd
%{_datadir}/dbus-1/system.d/com.chrisdaggas.NetworkManagerd.conf
%{_datadir}/dbus-1/system-services/com.chrisdaggas.NetworkManagerd.service
%{_datadir}/polkit-1/actions/com.chrisdaggas.network-manager.policy
%{_unitdir}/network-managerd.service

%files cli
%license LICENSE
%{_bindir}/nmctl

%changelog
* Sat Jan 10 2026 Christos A. Daggas <christosdaggas79@gmail.com> - 0.1.0-1
- Initial release
- GTK4/libadwaita GUI application
- Privileged D-Bus daemon with Polkit integration
- Command-line interface (nmctl)
- Profile management with network and system actions
- Auto-switch rule engine
- Support for 10 languages
