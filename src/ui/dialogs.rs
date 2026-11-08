//! About and keyboard shortcuts.

use adw::prelude::*;

pub fn about(window: &adw::ApplicationWindow) {
    adw::AboutDialog::builder()
        .application_name("Iconic")
        .application_icon(crate::APP_ID)
        .developer_name("UnderStackDev")
        .version(env!("CARGO_PKG_VERSION"))
        .comments("Icon themes and app launchers for GNOME")
        .website("https://github.com/UnderStackDev/gnome_iconic")
        .issue_url("https://github.com/UnderStackDev/gnome_iconic/issues")
        .license_type(gtk::License::MitX11)
        .copyright("© 2026 UnderStackDev")
        .build()
        .present(Some(window));
}

pub fn shortcuts(window: &adw::ApplicationWindow) {
    let dialog = adw::ShortcutsDialog::new();
    let sections: [(&str, &[(&str, &str)]); 3] = [
        ("General", &[
            ("Themes, Install, Apps", "1 2 3"),
            ("Search", "<Control>f"),
            ("Search", "slash"),
            ("Keyboard Shortcuts", "<Control>question"),
            ("Quit", "<Control>q"),
        ]),
        ("Themes", &[
            ("Use the picked theme", "Return"),
            ("Remove the picked theme you installed", "Delete"),
            ("Choose theme files to install", "<Control>o"),
        ]),
        ("Apps", &[
            ("Edit the picked app's name", "Return"),
            ("Pick an icon from the theme", "i"),
            ("Save the launcher", "<Control>s"),
            ("Undo", "<Control>z"),
            ("Redo", "<Control><Shift>z"),
        ]),
    ];
    for (title, items) in sections {
        let section = adw::ShortcutsSection::new(Some(title));
        for (label, accel) in items {
            section.add(adw::ShortcutsItem::new(label, accel));
        }
        dialog.add(section);
    }
    dialog.present(Some(window));
}
