//! The launcher editor beside the app list: name, icon and whether the app
//! shows in the app grid. An image dropped on the panel (from Files, say)
//! becomes the icon, so a theme folder can stay open while icons are tried.

use super::{ctx::Ctx, icon_picker};
use crate::core::entries::Entry;
use crate::core::{history, launchers};
use adw::prelude::*;
use gtk::{gdk, gio};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

pub struct EditorPanel {
    pub root: gtk::Stack,
    ctx: Rc<Ctx>,
    entry: RefCell<Option<Entry>>,
    icon: RefCell<String>,
    image: gtk::Image,
    name: adw::EntryRow,
    icon_row: adw::ActionRow,
    shown: adw::SwitchRow,
    path: gtk::Label,
    reset: gtk::Button,
    save: gtk::Button,
}

impl EditorPanel {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let image = gtk::Image::builder().pixel_size(128).css_classes(["icon-dropshadow"]).build();
        let frame = gtk::Box::builder().halign(gtk::Align::Center).css_classes(["card", "icon-drop"]).build();
        frame.append(&image);
        let hint = gtk::Label::builder().label("Drop a PNG or SVG here").css_classes(["caption", "dimmed"]).build();

        let name = adw::EntryRow::builder().title("Name").build();
        let icon_row = adw::ActionRow::builder().title("Icon").use_markup(false).subtitle_selectable(true).build();
        let from_theme = adw::ButtonRow::builder().title("From Theme…").build();
        let from_file = adw::ButtonRow::builder().title("From File…").build();
        let shown = adw::SwitchRow::builder().title("Show in App Grid").build();
        let (first, second, third) = (adw::PreferencesGroup::new(), adw::PreferencesGroup::new(), adw::PreferencesGroup::new());
        first.add(&name);
        second.add(&icon_row);
        second.add(&from_theme);
        second.add(&from_file);
        third.add(&shown);

        let path = gtk::Label::builder().css_classes(["caption", "dimmed"]).wrap(true).wrap_mode(gtk::pango::WrapMode::Char).xalign(0.0).selectable(true).build();
        let reset = gtk::Button::builder().label("Reset").css_classes(["destructive-action"]).build();
        let save = gtk::Button::builder().label("Save").css_classes(["suggested-action"]).hexpand(true).halign(gtk::Align::End).build();
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        buttons.append(&reset);
        buttons.append(&save);

        let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).css_classes(["editor-body"]).build();
        for w in [frame.upcast_ref::<gtk::Widget>(), hint.upcast_ref(), first.upcast_ref(), second.upcast_ref(), third.upcast_ref()] {
            body.append(w);
        }
        let footer = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).css_classes(["editor-footer"]).build();
        footer.append(&path);
        footer.append(&buttons);
        let editor = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor.append(&gtk::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).child(&body).build());
        editor.append(&footer);

        let empty = adw::StatusPage::builder().icon_name("view-app-grid-symbolic").title("No App Selected").description("Pick a launcher from the list to change it").css_classes(["compact"]).build();
        let root = gtk::Stack::new();
        root.add_named(&empty, Some("empty"));
        root.add_named(&editor, Some("editor"));

        let panel = Rc::new(EditorPanel { root, ctx: ctx.clone(), entry: RefCell::default(), icon: RefCell::default(), image, name, icon_row, shown, path, reset, save });
        let p = panel.clone();
        panel.save.connect_clicked(move |_| p.save());
        let p = panel.clone();
        panel.name.connect_entry_activated(move |_| p.save());
        let p = panel.clone();
        panel.reset.connect_clicked(move |_| p.reset());
        let p = panel.clone();
        from_theme.connect_activated(move |_| p.pick_from_theme());
        let p = panel.clone();
        from_file.connect_activated(move |_| p.pick_file());
        panel.add_drop(&editor);
        panel
    }

    /// Images dropped anywhere on the panel; the icon frame lights up.
    fn add_drop(self: &Rc<Self>, area: &gtk::Box) {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let p = self.clone();
        target.connect_drop(move |_, value, _, _| {
            let Ok(files) = value.get::<gdk::FileList>() else { return false };
            let Some(path) = files.files().first().and_then(|f| f.path()) else { return false };
            p.use_file(&path);
            true
        });
        area.add_controller(target);
    }

    /// Shows a launcher (or the empty page), dropping unsaved edits.
    pub fn show(&self, entry: Option<Entry>) {
        let Some(e) = entry else {
            self.root.set_visible_child_name("empty");
            *self.entry.borrow_mut() = None;
            return;
        };
        self.name.set_text(&e.name);
        self.shown.set_active(!e.hidden);
        self.shown.set_subtitle(if e.hidden_by_desktop { "Meant for another desktop, so GNOME hides it anyway" } else { "" });
        self.path.set_label(&e.path.to_string_lossy());
        self.reset.set_visible(e.modified());
        *self.icon.borrow_mut() = e.icon.clone();
        *self.entry.borrow_mut() = Some(e);
        self.show_icon();
        self.root.set_visible_child_name("editor");
    }

    pub fn id(&self) -> Option<String> {
        self.entry.borrow().as_ref().map(|e| e.id.clone())
    }

    fn show_icon(&self) {
        let icon = self.icon.borrow();
        if icon.starts_with('/') {
            self.image.set_from_file(Some(icon.as_str()));
        } else {
            self.image.set_icon_name(Some(icon.as_str()));
        }
        self.icon_row.set_subtitle(&icon);
    }

    fn set_icon(&self, icon: String) {
        *self.icon.borrow_mut() = icon;
        self.show_icon();
    }

    /// Copies a picked or dropped image to Iconic's icon folder and uses it.
    fn use_file(&self, path: &Path) {
        let Some(id) = self.id() else { return };
        match launchers::import_icon(path, &id) {
            Ok(stored) => self.set_icon(stored.to_string_lossy().into_owned()),
            Err(e) => self.ctx.error(&e),
        }
    }

    pub fn pick_from_theme(self: &Rc<Self>) {
        if self.id().is_none() {
            return;
        }
        let p = self.clone();
        icon_picker::open(&self.ctx.window, &self.ctx.current_theme(), move |name| p.set_icon(name));
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
        let p = self.clone();
        dialog.open(Some(&self.ctx.window), gio::Cancellable::NONE, move |result| {
            // Cancel is a normal answer, not an error.
            if let Some(path) = result.ok().and_then(|f| f.path()) {
                p.use_file(&path);
            }
        });
    }

    pub fn focus_name(&self) {
        self.name.grab_focus();
    }

    /// What differs from the saved launcher; None when nothing does.
    pub fn pending(&self) -> Option<launchers::Change> {
        let entry = self.entry.borrow();
        let e = entry.as_ref()?;
        let name = self.name.text().trim().to_string();
        let icon = self.icon.borrow().clone();
        let hidden = !self.shown.is_active();
        let change = launchers::Change {
            name: (!name.is_empty() && name != e.name).then_some(name),
            icon: (icon != e.icon).then_some(icon),
            hidden: (hidden != e.hidden).then_some(hidden),
        };
        (change.name.is_some() || change.icon.is_some() || change.hidden.is_some()).then_some(change)
    }

    pub fn save(&self) {
        let (Some(change), Some(e)) = (self.pending(), self.entry.borrow().clone()) else { return };
        run_change(&self.ctx, &e.id, &format!("Saved {}", e.name), || launchers::apply(&e, &change).map(|_| ()));
    }

    fn reset(&self) {
        let Some(e) = self.entry.borrow().clone() else { return };
        run_change(&self.ctx, &e.id, &format!("Reset {}", e.name), || launchers::reset(&e));
    }
}

/// Runs a launcher change with an undo point, then refreshes the list.
fn run_change(ctx: &Rc<Ctx>, id: &str, done: &str, f: impl FnOnce() -> anyhow::Result<()>) {
    let before = history::snapshot(id);
    let result = f();
    if result.is_ok() {
        ctx.history.borrow_mut().push(before);
    }
    ctx.reload_apps();
    match result {
        Ok(()) => ctx.toast_with_action(done, "Undo", "win.undo", None),
        Err(e) => ctx.error(&e),
    }
}
