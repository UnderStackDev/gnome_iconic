//! The desktop's icon theme: gsettings first, then the config files of
//! other toolkits that the user has set up.

use super::{ini, paths};
use anyhow::{Result, bail};
use gtk::gio;
use gtk::prelude::*;

const SCHEMA: &str = "org.gnome.desktop.interface";
const KEY: &str = "icon-theme";

pub fn interface_settings() -> gio::Settings {
    gio::Settings::new(SCHEMA)
}

pub fn current_theme() -> String {
    interface_settings().string(KEY).to_string()
}

/// A theme id is a folder name; anything else is refused.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
}

/// Makes `id` the icon theme. GNOME, GTK and Flatpak apps follow gsettings
/// right away; GTK, KDE and Qt config files are updated only when they exist.
pub fn apply(id: &str) -> Result<()> {
    if !valid_id(id) {
        bail!("Invalid theme name: {id}");
    }
    let settings = interface_settings();
    settings.set_string(KEY, id)?;
    gio::Settings::sync();
    sync_files(id)
}

fn sync_files(id: &str) -> Result<()> {
    let config = paths::config_home();
    let files = [
        (config.join("gtk-3.0/settings.ini"), "Settings", "gtk-icon-theme-name"),
        (config.join("gtk-4.0/settings.ini"), "Settings", "gtk-icon-theme-name"),
        (config.join("kdeglobals"), "Icons", "Theme"),
        (config.join("qt5ct/qt5ct.conf"), "Appearance", "icon_theme"),
        (config.join("qt6ct/qt6ct.conf"), "Appearance", "icon_theme"),
    ];
    for (file, section, key) in files {
        if file.exists() && ini::value(&file, section, key).as_deref() != Some(id) {
            ini::set_value(&file, section, key, id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_id;

    #[test]
    fn theme_ids() {
        assert!(valid_id("Papirus-Dark"));
        assert!(valid_id("Yaru-blue_2.0"));
        assert!(!valid_id("../evil"));
        assert!(!valid_id(".hidden"));
        assert!(!valid_id("a b"));
    }
}
