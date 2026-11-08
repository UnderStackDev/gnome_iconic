//! The Apps page: the launcher list on the left, filtered to all, modified
//! or hidden, and the editor for the picked launcher on the right. On a
//! narrow window the editor slides over the list.

use super::{ctx::Ctx, editor_panel::EditorPanel};
use crate::core::entries::Entry;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct AppsPage {
    pub root: adw::OverlaySplitView,
    pub search: gtk::SearchEntry,
    filter: adw::ToggleGroup,
    list: gtk::ListBox,
    editor: Rc<EditorPanel>,
    ids: RefCell<Vec<String>>,
    rebuilding: Cell<bool>,
    ctx: Rc<Ctx>,
}

/// A launcher's icon: Icon= is a theme name or an absolute file path.
pub fn icon_image(icon: &str, size: i32) -> gtk::Image {
    let image = if icon.starts_with('/') { gtk::Image::from_file(icon) } else { gtk::Image::from_icon_name(icon) };
    image.set_pixel_size(size);
    image
}

impl AppsPage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let search = gtk::SearchEntry::builder().hexpand(true).build();
        let filter = adw::ToggleGroup::new();
        for (name, label) in [("all", "All"), ("modified", "Modified"), ("hidden", "Hidden")] {
            filter.add(adw::Toggle::builder().name(name).label(label).build());
        }
        filter.set_active_name(Some("all"));
        let top = gtk::Box::builder().spacing(12).css_classes(["search-row"]).build();
        top.append(&search);
        top.append(&filter);

        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Browse).css_classes(["boxed-list", "page-body"]).valign(gtk::Align::Start).build();
        list.set_placeholder(Some(&gtk::Label::builder().label("No apps match").css_classes(["dimmed"]).margin_top(24).margin_bottom(24).build()));
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&top);
        column.append(&gtk::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).child(&list).build());

        let editor = EditorPanel::new(ctx);
        let root = adw::OverlaySplitView::builder()
            .content(&column)
            .sidebar(&editor.root)
            .sidebar_position(gtk::PackType::End)
            .min_sidebar_width(340.0)
            .max_sidebar_width(440.0)
            .sidebar_width_fraction(0.38)
            .build();

        let page = Rc::new(AppsPage { root, search, filter, list, editor, ids: RefCell::default(), rebuilding: Cell::new(false), ctx: ctx.clone() });
        let weak = Rc::downgrade(&page);
        page.search.connect_search_changed(move |_| weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        let weak = Rc::downgrade(&page);
        page.filter.connect_active_name_notify(move |_| weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        let weak = Rc::downgrade(&page);
        ctx.on_apps_changed(move || weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        // The list shows icons from the current theme.
        let weak = Rc::downgrade(&page);
        ctx.on_themes_changed(move || weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        let weak = Rc::downgrade(&page);
        page.list.connect_row_selected(move |_, row| {
            let Some(page) = weak.upgrade() else { return };
            if page.rebuilding.get() {
                return;
            }
            let id = row.and_then(|r| page.ids.borrow().get(r.index() as usize).cloned());
            page.pick(id);
        });
        let weak = Rc::downgrade(&page);
        page.list.connect_row_activated(move |_, _| {
            if let Some(page) = weak.upgrade() {
                page.root.set_show_sidebar(true);
                page.editor.focus_name();
            }
        });
        page.add_keys();
        page.rebuild();
        page
    }

    /// Ctrl+S saves and I picks an icon from the theme, anywhere on the page.
    fn add_keys(self: &Rc<Self>) {
        let keys = gtk::ShortcutController::new();
        let e = self.editor.clone();
        keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string("<Control>s"), Some(gtk::CallbackAction::new(move |_, _| {
            e.save();
            glib::Propagation::Stop
        }))));
        let e = self.editor.clone();
        keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string("i"), Some(gtk::CallbackAction::new(move |_, _| {
            e.pick_from_theme();
            glib::Propagation::Stop
        }))));
        self.root.add_controller(keys);
    }

    /// Shows launcher `id` in the editor, asking first about unsaved edits.
    fn pick(self: &Rc<Self>, id: Option<String>) {
        if id == self.editor.id() {
            return;
        }
        let entry = id.and_then(|id| self.ctx.apps.borrow().iter().find(|e| e.id == id).cloned());
        if self.editor.pending().is_none() {
            self.editor.show(entry);
            return;
        }
        let dialog = adw::AlertDialog::new(Some("Save Changes?"), Some("The launcher you were editing has unsaved changes."));
        dialog.add_responses(&[("discard", "Discard"), ("save", "Save")]);
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("discard");
        let page = self.clone();
        let entry = RefCell::new(entry);
        dialog.connect_response(None, move |_, response| {
            if response == "save" {
                page.editor.save();
            }
            let entry = entry.take();
            let id = entry.as_ref().map(|e| e.id.clone());
            page.editor.show(entry);
            // Saving rebuilt the list around the old launcher.
            if let Some(id) = id {
                page.select(&id);
            }
        });
        dialog.present(Some(&self.ctx.window));
    }

    fn shows(&self, e: &Entry) -> bool {
        let off = e.hidden || e.hidden_by_desktop;
        let wanted = match self.filter.active_name().as_deref() {
            Some("modified") => e.modified(),
            Some("hidden") => off,
            _ => !off || e.modified(),
        };
        let query = self.search.text().to_lowercase();
        wanted && (query.is_empty() || e.name.to_lowercase().contains(&query) || e.id.to_lowercase().contains(&query))
    }

    fn rebuild(self: &Rc<Self>) {
        self.rebuilding.set(true);
        self.list.remove_all();
        let apps = self.ctx.apps.borrow();
        self.search.set_placeholder_text(Some(&format!("Search {} apps", apps.len())));
        let shown: Vec<&Entry> = apps.iter().filter(|e| self.shows(e)).collect();
        *self.ids.borrow_mut() = shown.iter().map(|e| e.id.clone()).collect();
        let current = self.editor.id();
        for entry in &shown {
            let row = row(entry);
            self.list.append(&row);
            if current.as_deref() == Some(entry.id.as_str()) {
                self.list.select_row(Some(&row));
            }
        }
        self.rebuilding.set(false);
        // The launcher on show may have changed on disk; keep unsaved edits.
        if self.editor.pending().is_none() {
            self.editor.show(current.and_then(|id| apps.iter().find(|e| e.id == id).cloned()));
        }
    }

    pub fn focus_list(&self) {
        let row = self.list.selected_row().or_else(|| self.list.row_at_index(0));
        if let Some(row) = row {
            row.grab_focus();
        }
    }

    /// Picks a launcher by id.
    pub fn select(self: &Rc<Self>, id: &str) {
        let index = self.ids.borrow().iter().position(|i| i == id);
        if let Some(row) = index.and_then(|i| self.list.row_at_index(i as i32)) {
            self.list.select_row(Some(&row));
        }
    }
}

fn row(entry: &Entry) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(&entry.name).subtitle(&entry.id).use_markup(false).activatable(true).build();
    if !entry.comment.is_empty() {
        row.set_tooltip_text(Some(&entry.comment));
    }
    let icon = icon_image(&entry.icon, 32);
    icon.add_css_class("icon-dropshadow");
    row.add_prefix(&icon);
    if entry.hidden || entry.hidden_by_desktop {
        row.add_suffix(&badge("Hidden", "dimmed"));
    }
    if entry.modified() {
        row.add_suffix(&badge("Modified", "accent"));
    }
    row
}

fn badge(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder().label(text).css_classes(["badge", class]).valign(gtk::Align::Center).build()
}
