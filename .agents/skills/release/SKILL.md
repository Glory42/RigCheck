---
name: release
description: Cut a rigcheck release: bump the workspace version, update AppStream, tag, and refresh the AUR and Flatpak recipes. Use only when asked to release, bump the version, or update package recipes/publishing metadata.
---

# Release

Don't bump versions in ordinary PRs. Read `packaging/README.md` before changing
recipes; it is the source of truth for the package formats.

## 1. Prepare the version bump PR

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`); every crate
   inherits it.
2. Bump the hardcoded `rigcheck-*` path-dependency versions in the manifests
   that pin them: `app/Cargo.toml`, `crates/gamedb/Cargo.toml`,
   `crates/hardware/Cargo.toml`, `pipeline/Cargo.toml`. Search for the old
   version so none is missed:

   ```sh
   grep -rn '<old-version>' --include=Cargo.toml .
   ```

3. Add a `<release version="…" date="…">` entry to
   `packaging/dev.gorkemkaryol.rigcheck.metainfo.xml` (keep the previous
   entries) and mention what changed in its `<description>`.
4. Run `cargo build --workspace` (or `cargo check`) so `Cargo.lock` updates,
   then the usual CI checks.
5. Merge to `master` only when CI is green.

## 2. Tag and publish

The tag must match `Cargo.toml`:

```sh
git tag v<version>
git push origin v<version>
```

`.github/workflows/release-app.yml` builds the AppImage, tarball, `.deb`,
`.rpm` and `version.txt` and publishes them. Verify the release assets before
updating packaging.

## 3. AUR

`rigcheck-bin` (release tarball) after every release:

```sh
cd packaging/aur
# bump pkgver (and pkgrel back to 1)
updpkgsums
makepkg --printsrcinfo > .SRCINFO
makepkg -f          # check it builds
```

Commit the recipe here, then copy `PKGBUILD` and `.SRCINFO` to a clone of
`ssh://aur@aur.archlinux.org/rigcheck-bin.git`, commit and push `master`.

`rigcheck-git` builds from `master`, so it needs a push only when its recipe
changes (dependencies, install steps, `pkgver()`).

## 4. Flatpak

In `packaging/flatpak/dev.gorkemkaryol.rigcheck.yml`, point `tag:` at the new
release, then regenerate the crates from that tag's `Cargo.lock` with
`flatpak-cargo-generator.py` (see `packaging/README.md`). Build the manifest
locally if flatpak is available. `Flatpak` CI builds it on recipe PRs.

## 5. Website

The site reads the latest GitHub release at build time; it needs a rebuild to
show the new version. The follow-up PR that updates `packaging/aur/` triggers
that build. Check the download links on the deployed site after it lands.

## Never

- Don't push tags, publish to AUR, or open upstream Flathub PRs unless the user
  asked for that step.
- Don't hand-edit generated artifacts (`packaging/aur/src/`,
  `packaging/aur/pkg/`, `packaging/flatpak/cargo-sources.json` is generated,
  but it is committed).
- Don't ship a release with failing `cargo deny` advisories or a red CI.
