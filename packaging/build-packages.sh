#!/bin/bash
# Build script for CD Network Manager - RPM, DEB, and AppImage
# Copyright (C) 2026 Christos A. Daggas
# SPDX-License-Identifier: MIT

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# App configuration
APP_NAME="network-manager"
APP_ID="com.chrisdaggas.network-manager"
VERSION=$(grep -m1 'version = ' "$PROJECT_ROOT/Cargo.toml" | sed 's/.*version = "\([^"]*\)".*/\1/' | head -1)

# Determine architecture
HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
    x86_64)  DEB_ARCH="amd64"  ;;
    aarch64) DEB_ARCH="arm64"  ;;
    armv7l)  DEB_ARCH="armhf"  ;;
    *)       DEB_ARCH="$HOST_ARCH" ;;
esac
ARCH="$HOST_ARCH"

# Output directories
DIST_DIR="$PROJECT_ROOT/dist"
RPM_OUT="$DIST_DIR/rpm"
DEB_OUT="$DIST_DIR/deb"
APPIMAGE_OUT="$DIST_DIR/appimage"

# Work directories (use /tmp to keep dist clean)
DEB_WORK="/tmp/nm-deb-work-$$"
APPIMAGE_WORK="/tmp/nm-appimage-work-$$"
TOOLS_DIR="/tmp/nm-tools-$$"

echo -e "${GREEN}=== Building CD Network Manager Packages ===${NC}"
echo "Version: $VERSION"
echo "Project Root: $PROJECT_ROOT"
echo "Output: $DIST_DIR"

# Create output directories
mkdir -p "$RPM_OUT" "$DEB_OUT" "$APPIMAGE_OUT"

# Build release binary
echo -e "\n${YELLOW}=== Building Release Binary ===${NC}"
cd "$PROJECT_ROOT"
cargo build --release

# Verify binary was built
if [ ! -f "$PROJECT_ROOT/target/release/network-manager" ]; then
    echo -e "${RED}Error: Binary not found at target/release/network-manager${NC}"
    exit 1
fi
echo -e "${GREEN}Binary built successfully${NC}"

# Validate metadata files
echo -e "\n${YELLOW}=== Validating Metadata ===${NC}"
desktop-file-validate "$PROJECT_ROOT/data/${APP_ID}.desktop" || echo "Warning: Desktop file validation had issues"
appstream-util validate-relax "$PROJECT_ROOT/data/${APP_ID}.metainfo.xml" || echo "Warning: Metainfo validation had issues"

# =============================================================================
# RPM BUILD
# =============================================================================
echo -e "\n${YELLOW}=== Building RPM Package ===${NC}"

# Use /tmp for RPM build to avoid spaces in path
RPMBUILD_DIR="/tmp/rpmbuild-nm-$$"
rm -rf "$RPMBUILD_DIR"
mkdir -p "$RPMBUILD_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Store binary path for spec file
BINARY_PATH="$PROJECT_ROOT/target/release/network-manager"
DATA_DIR="$PROJECT_ROOT/data"

# Create spec file with absolute paths
cat > "$RPMBUILD_DIR/SPECS/${APP_NAME}.spec" << SPEC_EOF
%global debug_package %{nil}

Name:           network-manager
Version:        ${VERSION}
Release:        1%{?dist}
Summary:        Network and system profile manager for Linux

License:        MIT
URL:            https://chrisdaggas.com

Requires:       gtk4
Requires:       libadwaita

%description
CD Network Manager is a Linux-native system and network profile
manager. It allows you to create, manage, and quickly switch between
comprehensive network and system configuration profiles.

%install
# Install binary
install -Dm755 "${BINARY_PATH}" %{buildroot}%{_bindir}/network-manager

# Install desktop file
install -Dm644 "${DATA_DIR}/${APP_ID}.desktop" \\
    %{buildroot}%{_datadir}/applications/${APP_ID}.desktop

# Install metainfo
install -Dm644 "${DATA_DIR}/${APP_ID}.metainfo.xml" \\
    %{buildroot}%{_datadir}/metainfo/${APP_ID}.metainfo.xml

# Install icon
install -Dm644 "${DATA_DIR}/icons/hicolor/scalable/apps/${APP_ID}.svg" \\
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/${APP_ID}.svg

%files
%{_bindir}/network-manager
%{_datadir}/applications/${APP_ID}.desktop
%{_datadir}/metainfo/${APP_ID}.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/${APP_ID}.svg

%changelog
* Sat Jan 11 2025 Christos A. Daggas <info@chrisdaggas.com> - ${VERSION}-1
- Initial package release
SPEC_EOF

# Build RPM
rpmbuild --define "_topdir $RPMBUILD_DIR" \
         -bb "$RPMBUILD_DIR/SPECS/${APP_NAME}.spec"

# Copy RPM to output
find "$RPMBUILD_DIR/RPMS" -name "*.rpm" -exec cp {} "$RPM_OUT/" \;
echo -e "${GREEN}RPM packages built:${NC}"
ls -la "$RPM_OUT/"

# Cleanup temp dir
rm -rf "$RPMBUILD_DIR"

# =============================================================================
# DEB BUILD
# =============================================================================
echo -e "\n${YELLOW}=== Building DEB Package ===${NC}"

DEB_PKG_DIR="$DEB_WORK/${APP_NAME}_${VERSION}_amd64"
rm -rf "$DEB_PKG_DIR"
mkdir -p "$DEB_PKG_DIR/DEBIAN"
mkdir -p "$DEB_PKG_DIR/usr/bin"
mkdir -p "$DEB_PKG_DIR/usr/share/applications"
mkdir -p "$DEB_PKG_DIR/usr/share/metainfo"
mkdir -p "$DEB_PKG_DIR/usr/share/icons/hicolor/scalable/apps"

# Install binary
install -Dm755 "$PROJECT_ROOT/target/release/network-manager" "$DEB_PKG_DIR/usr/bin/network-manager"

# Install data files
install -Dm644 "$PROJECT_ROOT/data/${APP_ID}.desktop" "$DEB_PKG_DIR/usr/share/applications/${APP_ID}.desktop"
install -Dm644 "$PROJECT_ROOT/data/${APP_ID}.metainfo.xml" "$DEB_PKG_DIR/usr/share/metainfo/${APP_ID}.metainfo.xml"
install -Dm644 "$PROJECT_ROOT/data/icons/hicolor/scalable/apps/${APP_ID}.svg" \
    "$DEB_PKG_DIR/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"

# Calculate installed size
INSTALLED_SIZE=$(du -s "$DEB_PKG_DIR" | cut -f1)

# Create control file
cat > "$DEB_PKG_DIR/DEBIAN/control" << EOF
Package: network-manager
Version: ${VERSION}
Section: net
Priority: optional
Architecture: ${DEB_ARCH}
Installed-Size: ${INSTALLED_SIZE}
Depends: libgtk-4-1, libadwaita-1-0, network-manager
Recommends: bubblewrap
Maintainer: Christos A. Daggas <info@chrisdaggas.com>
Homepage: https://chrisdaggas.com
Description: Network and system profile manager for Linux
 CD Network Manager is a Linux-native system and network profile
 manager. It allows you to create, manage, and quickly switch between
 comprehensive network and system configuration profiles.
EOF

# Build DEB
dpkg-deb --build "$DEB_PKG_DIR" "$DEB_OUT/${APP_NAME}_${VERSION}_${DEB_ARCH}.deb"
echo -e "${GREEN}DEB package built:${NC}"
ls -la "$DEB_OUT/"

# =============================================================================
# APPIMAGE BUILD
# =============================================================================
echo -e "\n${YELLOW}=== Building AppImage ===${NC}"

# Download linuxdeploy if needed
mkdir -p "$TOOLS_DIR"
LINUXDEPLOY="$TOOLS_DIR/linuxdeploy-x86_64.AppImage"
if [ ! -f "$LINUXDEPLOY" ]; then
    echo "Downloading linuxdeploy..."
    wget -q -O "$LINUXDEPLOY" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
    chmod +x "$LINUXDEPLOY"
fi

# Setup AppDir
APPDIR="$APPIMAGE_WORK/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/metainfo"
mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"

# Install binary
install -Dm755 "$PROJECT_ROOT/target/release/network-manager" "$APPDIR/usr/bin/network-manager"

# Install desktop and metainfo
install -Dm644 "$PROJECT_ROOT/data/${APP_ID}.desktop" "$APPDIR/${APP_ID}.desktop"
install -Dm644 "$PROJECT_ROOT/data/${APP_ID}.desktop" "$APPDIR/usr/share/applications/${APP_ID}.desktop"
install -Dm644 "$PROJECT_ROOT/data/${APP_ID}.metainfo.xml" "$APPDIR/usr/share/metainfo/${APP_ID}.metainfo.xml"

# Install icon
install -Dm644 "$PROJECT_ROOT/data/icons/hicolor/scalable/apps/${APP_ID}.svg" "$APPDIR/${APP_ID}.svg"
install -Dm644 "$PROJECT_ROOT/data/icons/hicolor/scalable/apps/${APP_ID}.svg" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"

# Create symbolic link for .DirIcon
ln -sf "${APP_ID}.svg" "$APPDIR/.DirIcon"

# Create AppRun
cat > "$APPDIR/AppRun" << 'APPRUN_EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${HERE}/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GDK_BACKEND="${GDK_BACKEND:-x11,wayland}"
exec "${HERE}/usr/bin/network-manager" "$@"
APPRUN_EOF
chmod +x "$APPDIR/AppRun"

# Build AppImage
cd "$DIST_DIR"
export ARCH=x86_64
export NO_STRIP=1
export DISABLE_COPYRIGHT_FILES_DEPLOYMENT=1

"$LINUXDEPLOY" --appdir "$APPDIR" \
    --desktop-file "$APPDIR/${APP_ID}.desktop" \
    --icon-file "$APPDIR/${APP_ID}.svg" \
    --output appimage || true

# Find and move the AppImage
APPIMAGE_FILE=$(find "$DIST_DIR" -maxdepth 1 -name "*.AppImage" -type f | head -1)
if [ -n "$APPIMAGE_FILE" ] && [ -f "$APPIMAGE_FILE" ]; then
    mv "$APPIMAGE_FILE" "$APPIMAGE_OUT/"
    echo -e "${GREEN}AppImage built:${NC}"
    ls -la "$APPIMAGE_OUT/"
else
    echo -e "${RED}AppImage build may have failed${NC}"
fi

# =============================================================================
# CLEANUP
# =============================================================================
echo -e "\n${YELLOW}=== Cleaning up temporary files ===${NC}"
rm -rf "$DEB_WORK" "$APPIMAGE_WORK" "$TOOLS_DIR" 2>/dev/null || true

# =============================================================================
# SUMMARY
# =============================================================================
echo -e "\n${GREEN}=== Build Complete ===${NC}"
echo "Output directory: $DIST_DIR"
echo ""
echo "RPM packages:"
ls -la "$RPM_OUT/" 2>/dev/null || echo "  (none)"
echo ""
echo "DEB packages:"
ls -la "$DEB_OUT/" 2>/dev/null || echo "  (none)"
echo ""
echo "AppImage:"
ls -la "$APPIMAGE_OUT/" 2>/dev/null || echo "  (none)"
