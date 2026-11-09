//! The window. Plain libadwaita: it follows the system's light/dark style
//! and accent colour.

mod apps_page;
mod ctx;
mod dialogs;
mod editor_panel;
mod icon_picker;
mod install_page;
#[cfg(debug_assertions)]
mod snapshot;
mod theme_card;
mod themes_page;
mod window;

use adw::prelude::*;
use gtk::{gdk, glib};

const CSS: &str = "
.search-row { margin: 12px 18px 6px; }
.page-body { margin: 6px 18px 24px; }
.theme-grid > flowboxchild { padding: 0; border-radius: 15px; }
.theme-card { padding: 26px 16px 14px; }
.theme-grid .remove { min-width: 26px; min-height: 26px; padding: 0; }
.theme-grid .remove { opacity: 0; transition: opacity 150ms; }
flowboxchild:hover .remove, flowboxchild:selected .remove, .remove:focus { opacity: 1; }
.theme-card.active { box-shadow: inset 0 0 0 2px var(--accent-bg-color); }
.editor-body { margin: 18px; }
.editor-footer { margin: 0 18px 18px; }
.icon-drop { padding: 20px; border-radius: 24px; transition: box-shadow 150ms; }
.icon-drop:drop(active) { box-shadow: inset 0 0 0 2px var(--accent-bg-color); }
.badge { font-size: 0.8em; font-weight: bold; padding: 2px 8px; border-radius: 99px; }
.badge.accent { color: var(--accent-color); background: color-mix(in srgb, var(--accent-bg-color) 15%, transparent); }
.badge.dimmed { background: alpha(currentColor, 0.1); }
";

/// Centres a widget at a readable width, with a CSS class for margins.
pub fn clamped(child: &impl IsA<gtk::Widget>, class: &str) -> adw::Clamp {
    adw::Clamp::builder().maximum_size(1200).tightening_threshold(900).child(child).css_classes([class]).build()
}

pub fn run() -> i32 {
    let app = adw::Application::builder().application_id(crate::APP_ID).build();
    // Debug builds run beside an installed Iconic instead of handing over to it.
    #[cfg(debug_assertions)]
    app.set_flags(gtk::gio::ApplicationFlags::NON_UNIQUE);
    app.connect_startup(|app| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("no display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let quit = gtk::gio::SimpleAction::new("quit", None);
        quit.connect_activate(glib::clone!(#[weak] app, move |_, _| app.quit()));
        app.add_action(&quit);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        app.set_accels_for_action("win.search", &["<Control>f"]);
        app.set_accels_for_action("win.shortcuts", &["<Control>question"]);
        app.set_accels_for_action("win.install-files", &["<Control>o"]);
        app.set_accels_for_action("win.undo", &["<Control>z"]);
        app.set_accels_for_action("win.redo", &["<Control><Shift>z"]);
    });
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => {
            let window = window::build(app);
            window.present();
            #[cfg(debug_assertions)]
            snapshot::maybe_capture(&window);
        }
    });
    // Arguments were handled by the command line already.
    app.run_with_args::<&str>(&[]).get() as i32
}
