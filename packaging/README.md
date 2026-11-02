# Packaging

Files that install rigcheck into a Linux desktop. The app ID, `dev.gorkemkaryol.rigcheck`, names all of them: Wayland matches the running window to its `.desktop` file (and so its icon) by this ID, and Flatpak uses it as the app ID.

| File | Installed to |
|---|---|
| `dev.gorkemkaryol.rigcheck.desktop` | `share/applications/` |
| `dev.gorkemkaryol.rigcheck.metainfo.xml` | `share/metainfo/` (AppStream: what software centers show) |
| `assets/brand/icon-{128,256,512}.png` | `share/icons/hicolor/<size>x<size>/apps/dev.gorkemkaryol.rigcheck.png` |

CI validates the first two (`desktop-file-validate`, `appstreamcli validate`).
