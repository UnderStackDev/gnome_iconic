//! The Apps page: every launcher, searchable, filtered to all, modified or
//! hidden. Activating one opens the editor.

use super::{app_editor, ctx::Ctx};
use crate::core::entries::Entry;
use adw::prelude::*;
use std::rc::Rc;

pub struct AppsPage {
    pub root: gtk::Box,
    pub search: gtk::SearchEntry,
    filter: adw::ToggleGroup,
    list: gtk::ListBox,
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
        let search = gtk::SearchEntry::builder().placeholder_text("Search apps").hexpand(true).build();
        let filter = adw::ToggleGroup::new();
        for (name, label) in [("all", "All"), ("modified", "Modified"), ("hidden", "Hidden")] {
            filter.add(adw::Toggle::builder().name(name).label(label).build());
        }
        filter.set_active_name(Some("all"));
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        top.append(&search);
        top.append(&filter);

        let list = gtk::ListBox::builder().css_classes(["boxed-list"]).valign(gtk::Align::Start).build();
        list.set_placeholder(Some(&gtk::Label::builder().label("No apps match").css_classes(["dimmed"]).margin_top(24).margin_bottom(24).build()));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&super::clamped(&top, "search-row"));
        root.append(
            &gtk::ScrolledWindow::builder()
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&super::clamped(&list, "page-body"))
                .build(),
        );

        let page = Rc::new(AppsPage { root, search, filter, list, ctx: ctx.clone() });
        let weak = Rc::downgrade(&page);
        page.search.connect_search_changed(move |_| weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        let weak = Rc::downgrade(&page);
        page.filter.connect_active_name_notify(move |_| weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        let weak = Rc::downgrade(&page);
        ctx.on_apps_changed(move || weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        // The list shows icons from the current theme.
        let weak = Rc::downgrade(&page);
        ctx.on_themes_changed(move || weak.upgrade().into_iter().for_each(|p| p.rebuild()));
        page.rebuild();
        page
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
        self.list.remove_all();
        for entry in self.ctx.apps.borrow().iter().filter(|e| self.shows(e)) {
            let row = adw::ActionRow::builder()
                .title(&entry.name)
                .subtitle(&entry.id)
                .use_markup(false)
                .activatable(true)
                .build();
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
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let weak = Rc::downgrade(self);
            let id = entry.id.clone();
            row.connect_activated(move |_| {
                if let Some(page) = weak.upgrade() {
                    app_editor::open(&page.ctx, &id);
                }
            });
            self.list.append(&row);
        }
    }

    pub fn focus_list(&self) {
        if let Some(row) = self.list.row_at_index(0) {
            row.grab_focus();
        }
    }
}

fn badge(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder().label(text).css_classes(["badge", class]).valign(gtk::Align::Center).build()
}
