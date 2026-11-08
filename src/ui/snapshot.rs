//! Debug builds only: ICONIC_SNAPSHOT=/path.png renders the window to a PNG
//! and quits (ICONIC_SNAPSHOT_PAGE picks the page). Wayland blocks screen
//! grabs from the command line, so this is how UI changes get checked.

use adw::prelude::*;
use gtk::glib;
use std::time::Duration;

pub fn maybe_capture(window: &adw::ApplicationWindow) {
    let Ok(path) = std::env::var("ICONIC_SNAPSHOT") else { return };
    if let Ok(page) = std::env::var("ICONIC_SNAPSHOT_PAGE") {
        WidgetExt::activate_action(window, "win.page", Some(&page.to_variant())).expect("win.page exists");
    }
    if let Ok(id) = std::env::var("ICONIC_SNAPSHOT_EDIT") {
        WidgetExt::activate_action(window, "win.debug-edit", Some(&id.to_variant())).expect("debug-edit exists");
    }
    let window = window.clone();
    glib::timeout_add_local_once(Duration::from_millis(2500), move || {
        let (w, h) = (window.width() as f64, window.height() as f64);
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, w, h);
        let node = snapshot.to_node().expect("window drew nothing");
        let renderer = window.renderer().expect("window has no renderer");
        renderer.render_texture(node, None).save_to_png(&path).expect("could not save the PNG");
        window.application().expect("app").quit();
    });
}
