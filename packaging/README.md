# Packaging

Files that install rigcheck into a Linux desktop. The app ID, `dev.gorkemkaryol.rigcheck`, names all of them: Wayland matches the running window to its `.desktop` file (and so its icon) by this ID, and Flatpak uses it as the app ID.

| File | Installed to |
|---|---|
| `dev.gorkemkaryol.rigcheck.desktop` | `share/applications/` |
| `dev.gorkemkaryol.rigcheck.metainfo.xml` | `share/metainfo/` (AppStream: what software centers show) |
| `assets/brand/icon-{128,256,512}.png` | `share/icons/hicolor/<size>x<size>/apps/dev.gorkemkaryol.rigcheck.png` |

CI validates the first two (`desktop-file-validate`, `appstreamcli validate`).

## Release packages

`.github/workflows/release-app.yml` builds them on every pull request that touches packaging (to catch breakage) and publishes them when a `v*` tag is pushed. The tag must match the version in `Cargo.toml`.

| Package | Built by |
|---|---|
| `rigcheck-<version>-x86_64.AppImage` | `packaging/package.sh` + `appimagetool` |
| `rigcheck-<version>-x86_64-linux.tar.gz` | `packaging/package.sh` |
| `rigcheck_<version>-1_amd64.deb` | `cargo deb` (`[package.metadata.deb]` in `app/Cargo.toml`) |
| `rigcheck-<version>-1.x86_64.rpm` | `cargo generate-rpm` (`[package.metadata.generate-rpm]`) |

Built on Ubuntu 22.04, so the binary runs on any distro with glibc 2.35 or newer. It links only glibc and libgcc; Wayland/X11 and Vulkan are loaded at runtime.

To release: bump `version` in `Cargo.toml` (and the `<releases>` entry in the metainfo), merge, then `git tag v<version> && git push origin v<version>`.

## AUR: `rigcheck-bin`

`aur/PKGBUILD` repackages the release tarball. After a release: bump `pkgver`, run `updpkgsums` and `makepkg --printsrcinfo > .SRCINFO`, then push both files to `ssh://aur@aur.archlinux.org/rigcheck-bin.git` (needs an AUR account with your SSH key).

## Flathub

`flatpak/dev.gorkemkaryol.rigcheck.yml` builds from the release tag, offline. Before submitting, generate the crates it needs:

```sh
# github.com/flatpak/flatpak-builder-tools, cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/dev.gorkemkaryol.rigcheck.yml
```

Then follow Flathub's submission guide (a pull request to `flathub/flathub` with the manifest and `cargo-sources.json`). In the sandbox `vulkaninfo` isn't available, so the Vulkan version and VRAM show as unknown; Steam libraries on other drives need an extra filesystem grant.
