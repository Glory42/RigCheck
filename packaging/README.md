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
| `version.txt` | `packaging/package.sh`; the app's update notice reads it |

Built on Ubuntu 22.04, so the binary runs on any distro with glibc 2.35 or newer. It links only glibc and libgcc; Wayland/X11 and Vulkan are loaded at runtime.

To release: bump `version` in `Cargo.toml` (and the `<releases>` entry in the metainfo), merge, then `git tag v<version> && git push origin v<version>`.

## AUR

Two packages, so Arch users pick a channel:

| Package | Channel | Recipe | Builds |
|---|---|---|---|
| `rigcheck-bin` | Stable | `aur/PKGBUILD` | Repackages the release tarball |
| `rigcheck-git` | Latest `master` | `aur-git/PKGBUILD` | Compiles from source on the user's machine; `pkgver` comes from `git describe` (`0.8.0.r2.g5f46a4e`), so users rebuild with `yay -Syu --devel` |

Both provide and conflict with `rigcheck`, so only one is installed at a time. Each is published by pushing its `PKGBUILD` and `.SRCINFO` to `ssh://aur@aur.archlinux.org/<package>.git` (needs an AUR account with your SSH key; the first push creates the package; the AUR only takes `master`):

```sh
git clone ssh://aur@aur.archlinux.org/rigcheck-bin.git /tmp/aur-rigcheck-bin
cp packaging/aur/{PKGBUILD,.SRCINFO} /tmp/aur-rigcheck-bin/
cd /tmp/aur-rigcheck-bin && git add PKGBUILD .SRCINFO && git commit -m "Update to <version>" && git push
```

After each release, update `rigcheck-bin` only: in `packaging/aur/`, bump `pkgver`, run `updpkgsums` and `makepkg --printsrcinfo > .SRCINFO`, check it with `makepkg -f`, commit here, then push both files to the AUR as above. `rigcheck-git` needs pushing again only when its recipe changes (new dependencies, install steps).

`rigcheck-git` sets `options=('!lto')`: makepkg's LTO compiles aws-lc's C code (under rustls) into GCC bitcode that Rust's linker can't read.

## Flathub

`flatpak/dev.gorkemkaryol.rigcheck.yml` builds from the release tag, offline, with the crates listed in `flatpak/cargo-sources.json`. The `Flatpak` workflow builds it on pull requests that touch it.

After each release, point the manifest at the new tag and regenerate the crates from that tag's `Cargo.lock`:

```sh
# github.com/flatpak/flatpak-builder-tools, cargo/flatpak-cargo-generator.py (needs aiohttp, tomlkit)
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --install --install-deps-from=flathub --force-clean build-dir packaging/flatpak/dev.gorkemkaryol.rigcheck.yml
```

To submit: fork `flathub/flathub`, branch from `new-pr`, add the manifest and `cargo-sources.json`, and open a pull request against `new-pr`. Once accepted, Flathub creates `flathub/dev.gorkemkaryol.rigcheck`, and updates are pull requests there. In the sandbox `vulkaninfo` isn't available, so the Vulkan version and VRAM show as unknown; Steam libraries on other drives need an extra filesystem grant.
