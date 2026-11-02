#!/usr/bin/env bash
# Builds the AppImage and the tarball from target/release/rigcheck into dist/.
# The .deb and .rpm come from `cargo deb` and `cargo generate-rpm` (see
# app/Cargo.toml). Usage: packaging/package.sh <version> [path to appimagetool]
set -euo pipefail

version="$1"
appimagetool="${2:-appimagetool}"
id=dev.gorkemkaryol.rigcheck
root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/dist"
rm -rf "$dist" && mkdir -p "$dist"

# The installed layout, shared by the tarball and the AppImage.
stage() {
    local into="$1"
    install -Dm755 "$root/target/release/rigcheck" "$into/usr/bin/rigcheck"
    install -Dm644 "$root/packaging/$id.desktop" "$into/usr/share/applications/$id.desktop"
    install -Dm644 "$root/packaging/$id.metainfo.xml" "$into/usr/share/metainfo/$id.metainfo.xml"
    for size in 128 256 512; do
        install -Dm644 "$root/assets/brand/icon-$size.png" \
            "$into/usr/share/icons/hicolor/${size}x${size}/apps/$id.png"
    done
}

# Tarball, for the AUR package and anyone installing by hand.
tarball="rigcheck-$version-x86_64-linux"
stage "$dist/$tarball"
install -Dm644 "$root/LICENSE" "$dist/$tarball/usr/share/licenses/rigcheck/LICENSE"
tar -C "$dist" -czf "$dist/$tarball.tar.gz" "$tarball"
rm -rf "$dist/$tarball"

# AppImage: the same layout plus what appimagetool wants at the top.
appdir="$dist/rigcheck.AppDir"
stage "$appdir"
cp "$root/packaging/$id.desktop" "$appdir/$id.desktop"
cp "$root/assets/brand/icon-256.png" "$appdir/$id.png"
ln -s usr/bin/rigcheck "$appdir/AppRun"
# No FUSE in containers and CI: let appimagetool unpack itself instead.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$appimagetool" --no-appstream "$appdir" \
    "$dist/rigcheck-$version-x86_64.AppImage"
rm -rf "$appdir"

# What the app's update notice reads (app/src/release.rs).
echo "$version" > "$dist/version.txt"

ls -l "$dist"
