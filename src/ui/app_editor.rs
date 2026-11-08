//! The launcher editor: name, icon (from the theme or a file) and whether
//! the app shows in the app grid. Saves only what changed.

use super::{apps_page::icon_image, ctx::Ctx, icon_picker};
use crate::core::entries::Entry;
use crate::core::{history, launchers};
use adw::prelude::*;
use gtk::gio;
use std::cell::RefCell;
use std::rc::Rc;

struct Editor {
    ctx: Rc<Ctx>,
    entry: Entry,
    dialog: adw::Dialog,
    name: adw::EntryRow,
    icon: RefCell<String>,
    icon_row: adw::ActionRow,
    preview: gtk::Box,
    shown: adw::SwitchRow,
}

pub fn open(ctx: &Rc<Ctx>, id: &str) {
    let Some(entry) = ctx.apps.borrow().iter().find(|e| e.id == id).cloned() else { return };
    let dialog = adw::Dialog::builder().title(&entry.name).content_width(460).build();
    let name = adw::EntryRow::builder().title("Name").text(&entry.name).build();
    let icon_row = adw::ActionRow::builder().title("Icon").subtitle(&entry.icon).use_markup(false).subtitle_selectable(true).build();
    let from_theme = gtk::Button::builder().label("From Theme…").valign(gtk::Align::Center).build();
    let from_file = gtk::Button::builder().label("From File…").valign(gtk::Align::Center).build();
    icon_row.add_suffix(&from_theme);
    icon_row.add_suffix(&from_file);
    let shown = adw::SwitchRow::builder().title("Show in App Grid").active(!entry.hidden).build();
    if entry.hidden_by_desktop {
        shown.set_subtitle("This app is meant for another desktop, so GNOME hides it anyway");
    }

    let group = adw::PreferencesGroup::new();
    group.add(&name);
    group.add(&icon_row);
    group.add(&shown);
    let preview = gtk::Box::builder().halign(gtk::Align::Center).build();
    let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(24).margin_top(12).margin_bottom(24).margin_start(24).margin_end(24).build();
    body.append(&preview);
    body.append(&group);
    if entry.modified() {
        let reset = gtk::Button::builder().label("Reset to Original").halign(gtk::Align::Center).css_classes(["pill", "destructive-action"]).build();
        body.append(&reset);
        let (ctx, e, d) = (ctx.clone(), entry.clone(), dialog.clone());
        reset.connect_clicked(move |_| {
            d.close();
            run_change(&ctx, &e.id, &format!("Reset {}", e.name), || launchers::reset(&e));
        });
    }

    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::builder().label("Save").css_classes(["suggested-action"]).build();
    let header = adw::HeaderBar::builder().show_start_title_buttons(false).show_end_title_buttons(false).build();
    header.pack_start(&cancel);
    header.pack_end(&save);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&body));
    dialog.set_child(Some(&view));
    dialog.set_default_widget(Some(&save));

    let editor = Rc::new(Editor { ctx: ctx.clone(), icon: RefCell::new(entry.icon.clone()), entry, dialog: dialog.clone(), name, icon_row, preview, shown });
    editor.show_icon();
    let d = dialog.clone();
    cancel.connect_clicked(move |_| {
        d.close();
    });
    let e = editor.clone();
    save.connect_clicked(move |_| e.save());
    let e = editor.clone();
    editor.name.connect_entry_activated(move |_| e.save());
    let e = editor.clone();
    from_theme.connect_clicked(move |_| {
        let e2 = e.clone();
        icon_picker::open(&e.dialog, &e.ctx.current_theme(), move |name| e2.set_icon(name));
    });
    let e = editor.clone();
    from_file.connect_clicked(move |_| e.pick_file());
    let shortcut = gtk::ShortcutController::new();
    let e = editor.clone();
    shortcut.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control>s"),
        Some(gtk::CallbackAction::new(move |_, _| {
            e.save();
            gtk::glib::Propagation::Stop
        })),
    ));
    dialog.add_controller(shortcut);
    dialog.present(Some(&ctx.window));
}

impl Editor {
    fn show_icon(&self) {
        while let Some(child) = self.preview.first_child() {
            self.preview.remove(&child);
        }
        let image = icon_image(&self.icon.borrow(), 96);
        image.add_css_class("icon-dropshadow");
        self.preview.append(&image);
        self.icon_row.set_subtitle(&self.icon.borrow());
    }

    fn set_icon(&self, icon: String) {
        *self.icon.borrow_mut() = icon;
        self.show_icon();
    }

    fn pick_file(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        for mime in ["image/png", "image/svg+xml", "image/x-xpixmap"] {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder().title("Choose an Icon").filters(&filters).modal(true).build();
        let editor = self.clone();
        dialog.open(Some(&self.ctx.window), gio::Cancellable::NONE, move |result| {
            let Some(path) = result.ok().and_then(|f| f.path()) else { return };
            match launchers::import_icon(&path, &editor.entry.id) {
                Ok(stored) => editor.set_icon(stored.to_string_lossy().into_owned()),
                Err(e) => editor.ctx.error(&e),
            }
        });
    }

    fn save(&self) {
        let e = &self.entry;
        let name = self.name.text().trim().to_string();
        let icon = self.icon.borrow().clone();
        let hidden = !self.shown.is_active();
        let change = launchers::Change {
            name: (!name.is_empty() && name != e.name).then_some(name),
            icon: (icon != e.icon).then_some(icon),
            hidden: (hidden != e.hidden).then_some(hidden),
        };
        self.dialog.close();
        if change.name.is_none() && change.icon.is_none() && change.hidden.is_none() {
            return;
        }
        run_change(&self.ctx, &e.id, &format!("Saved {}", e.name), || launchers::apply(e, &change).map(|_| ()));
    }
}

/// Runs a launcher change with an undo point, then refreshes the list.
fn run_change(ctx: &Rc<Ctx>, id: &str, done: &str, f: impl FnOnce() -> anyhow::Result<()>) {
    let before = history::snapshot(id);
    match f() {
        Ok(()) => {
            ctx.history.borrow_mut().push(before);
            ctx.reload_apps();
            ctx.toast_with_action(done, "Undo", "win.undo", None);
        }
        Err(e) => {
            ctx.reload_apps();
            ctx.error(&e);
        }
    }
}
