# Iconic for GNOME

Icon themes and app launchers for GNOME. A port of
[Iconic for Omarchy](https://github.com/UnderStackDev/omarchy_iconic), written in
Rust with GTK 4 and libadwaita, so it follows your light/dark style and accent
colour.

- **Themes**: pick the desktop's icon theme from previews of every installed
  theme. GNOME, GTK and Flatpak apps switch right away.
- **Install**: drop an icon theme archive or folder anywhere on the window. It
  lands in `~/.local/share/icons`, where Flatpak apps can see it too.
- **Apps**: rename a launcher, give it another icon, or hide it from the app
  grid, without touching the package's own files.

## Why there is no icon layer here

Omarchy's launcher doesn't look app icons up in the active theme, so the
Omarchy version builds an extra icon layer. GNOME Shell's app grid and dash use
the active theme directly, so this version doesn't need one. PNG themes such as
Yaru work as they are.

The Omarchy version's Follow/Lock mode is left out too: GNOME has no Omarchy
theme switch to follow.

## Install

```sh
cargo build --release
cargo deb --no-build          # target/debian/iconic_*.deb
cargo generate-rpm            # target/generate-rpm/iconic-*.rpm
sudo apt install ./target/debian/iconic_*.deb
```

Runtime needs: GTK 4, libadwaita 1.8, `bsdtar` (`libarchive-tools` on
Ubuntu), `gtk-update-icon-cache`, `update-desktop-database`.

## Keys

| Key | Action |
|---|---|
| `1` `2` `3` (or `Alt` + number) | Themes, Install, Apps |
| Arrows, `Enter` / `Space` | Pick and use a theme |
| `Ctrl+F` or `/` | Search |
| `Delete` | Remove the picked theme you installed |
| `Ctrl+O` | Choose theme files to install |
| `Ctrl+S` | Save the launcher (in the editor) |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / redo launcher changes |
| `Ctrl+?` | All keys |
| `Ctrl+Q` | Quit |

## Command line

Everything also works without the window. Every command takes `--json` and
exits 0 or 1.

```sh
iconic status
iconic list-themes
iconic apply Papirus-Dark
iconic install ~/Downloads/theme.tar.xz [--replace|--keep-both] [--apply]
iconic app list [--modified|--hidden]
iconic app set firefox --name "Web" --icon web-browser [--hide|--show]
iconic app reset firefox
```

## What it changes

| Where | What |
|---|---|
| gsettings `org.gnome.desktop.interface icon-theme` | the icon theme (GNOME, GTK, and Flatpak apps through the settings portal) |
| `~/.config/gtk-3.0/settings.ini`, `gtk-4.0/settings.ini`, `kdeglobals`, `qt5ct`/`qt6ct` | the same theme name, only if the file already exists |
| `~/.local/share/icons/` | themes you install |
| `~/.local/share/applications/` | launcher changes: a copy marked `X-Iconic-Override=true` for package and Flatpak apps; your own launchers are edited in place after a backup in `~/.local/share/iconic/backups/` |
| `~/.local/share/iconic/icons/` | icon files you picked for launchers |

Archives are listed before they're unpacked (2 GiB / 300,000 files at most),
unpacked by bsdtar, and only files and links that stay inside the archive are
installed.

## Development

```sh
cargo test
cargo run
# Debug builds can render the window to a PNG (Wayland blocks screen grabs):
ICONIC_SNAPSHOT=/tmp/shot.png ICONIC_SNAPSHOT_PAGE=apps cargo run
```

## License

MIT © 2026 UnderStackDev
