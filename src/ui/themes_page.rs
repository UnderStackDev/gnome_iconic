//! The Themes page: a searchable grid of every installed icon theme.
//! Click, Space or Enter applies one; Delete removes a user theme.

use super::ctx::Ctx;
use super::theme_card;
use crate::core::removal;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ThemesPage {
    pub root: gtk::Box,
    pub search: gtk::SearchEntry,
    flow: gtk::FlowBox,
    ids: RefCell<Vec<String>>,
    ctx: Rc<Ctx>,
}

impl ThemesPage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let search = gtk::SearchEntry::builder().placeholder_text("Search themes").hexpand(true).build();
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(6)
            .row_spacing(12)
            .column_spacing(12)
            .valign(gtk::Align::Start)
            .css_classes(["theme-grid"])
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&super::clamped(&search, "search-row"));
        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&super::clamped(&flow, "page-body"))
            .build();
        root.append(&scroller);

        let page = Rc::new(ThemesPage { root, search, flow, ids: RefCell::default(), ctx: ctx.clone() });
        let weak = Rc::downgrade(&page);
        page.flow.set_filter_func(move |child| {
            let Some(page) = weak.upgrade() else { return true };
            let query = page.search.text().to_lowercase();
            let ids = page.ids.borrow();
            let Some(id) = ids.get(child.index() as usize) else { return true };
            let themes = page.ctx.themes.borrow();
            let name = themes.iter().find(|t| &t.id == id).map(|t| t.name.to_lowercase()).unwrap_or_default();
            query.is_empty() || id.to_lowercase().contains(&query) || name.contains(&query)
        });
        let weak = Rc::downgrade(&page);
        page.search.connect_search_changed(move |_| {
            if let Some(page) = weak.upgrade() {
                page.flow.invalidate_filter();
            }
        });
        let weak = Rc::downgrade(&page);
        page.flow.connect_child_activated(move |_, child| {
            if let Some(page) = weak.upgrade() {
                let id = page.ids.borrow().get(child.index() as usize).cloned();
                if let Some(id) = id {
                    page.ctx.apply_theme(&id);
                }
            }
        });
        let weak = Rc::downgrade(&page);
        ctx.on_themes_changed(move || {
            if let Some(page) = weak.upgrade() {
                page.rebuild();
            }
        });
        page.rebuild();
        page
    }

    fn rebuild(self: &Rc<Self>) {
        let keep = self.picked().unwrap_or_else(|| self.ctx.current_theme());
        self.flow.remove_all();
        let current = self.ctx.current_theme();
        let themes = self.ctx.themes.borrow().clone();
        *self.ids.borrow_mut() = themes.iter().map(|t| t.id.clone()).collect();
        for theme in &themes {
            let card = theme_card::build(theme, theme.id == current);
            let weak = Rc::downgrade(self);
            let id = theme.id.clone();
            card.remove.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.confirm_remove(&id);
                }
            });
            self.flow.append(&card.child);
            if theme.id == keep {
                self.flow.select_child(&card.child);
            }
        }
    }

    /// The theme under the keyboard focus, or the selected one.
    fn picked(&self) -> Option<String> {
        let child = self
            .flow
            .focus_child()
            .and_downcast::<gtk::FlowBoxChild>()
            .or_else(|| self.flow.selected_children().into_iter().next())?;
        self.ids.borrow().get(child.index() as usize).cloned()
    }

    /// Puts the keyboard focus on the picked theme, for arrow keys.
    pub fn focus_grid(&self) {
        if let Some(child) = self.flow.selected_children().first() {
            child.grab_focus();
        }
    }

    pub fn remove_picked(self: &Rc<Self>) {
        if let Some(id) = self.picked() {
            self.confirm_remove(&id);
        }
    }

    fn confirm_remove(self: &Rc<Self>, id: &str) {
        let themes = self.ctx.themes.borrow().clone();
        let current = self.ctx.current_theme();
        if !removal::can_remove(&themes, id, &current) {
            self.ctx.toast("Only themes you installed can be removed, and not the one in use");
            return;
        }
        let theme = themes.iter().find(|t| t.id == id).expect("checked above");
        let dialog = adw::AlertDialog::new(
            Some(&format!("Remove {}?", theme.name)),
            Some(&format!("The theme's files are deleted from {}.", theme.path.display())),
        );
        dialog.add_responses(&[("cancel", "Cancel"), ("remove", "Remove")]);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        let page = self.clone();
        let id = id.to_string();
        dialog.connect_response(Some("remove"), move |_, _| {
            let themes = page.ctx.themes.borrow().clone();
            match removal::remove(&themes, &id, &page.ctx.current_theme()) {
                Ok(()) => {
                    page.ctx.reload_themes();
                    page.ctx.toast(&format!("Removed {id}"));
                }
                Err(e) => page.ctx.error(&e),
            }
        });
        dialog.present(Some(&self.ctx.window));
    }
}
