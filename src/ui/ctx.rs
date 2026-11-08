//! State shared by the window's pages, with change listeners.

use crate::core::entries::{self, Entry};
use crate::core::history::History;
use crate::core::{settings, themes};
use adw::prelude::*;
use gtk::gio;
use std::cell::RefCell;
use std::rc::Rc;

type Listener = Box<dyn Fn()>;

pub struct Ctx {
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub themes: RefCell<Vec<themes::ThemeInfo>>,
    pub current: RefCell<String>,
    pub apps: RefCell<Vec<Entry>>,
    pub history: RefCell<History>,
    settings: gio::Settings,
    theme_listeners: RefCell<Vec<Listener>>,
    app_listeners: RefCell<Vec<Listener>>,
}

impl Ctx {
    pub fn new(window: adw::ApplicationWindow, toasts: adw::ToastOverlay) -> Rc<Self> {
        let ctx = Rc::new(Ctx {
            window,
            toasts,
            themes: RefCell::new(themes::scan()),
            current: RefCell::new(settings::current_theme()),
            apps: RefCell::new(entries::scan()),
            history: RefCell::default(),
            settings: settings::interface_settings(),
            theme_listeners: RefCell::default(),
            app_listeners: RefCell::default(),
        });
        // The theme may also change from GNOME Tweaks or the command line.
        let weak = Rc::downgrade(&ctx);
        ctx.settings.connect_changed(Some("icon-theme"), move |s, _| {
            if let Some(ctx) = weak.upgrade() {
                *ctx.current.borrow_mut() = s.string("icon-theme").to_string();
                ctx.notify_themes();
            }
        });
        ctx
    }

    pub fn on_themes_changed(&self, f: impl Fn() + 'static) {
        self.theme_listeners.borrow_mut().push(Box::new(f));
    }

    pub fn on_apps_changed(&self, f: impl Fn() + 'static) {
        self.app_listeners.borrow_mut().push(Box::new(f));
    }

    fn notify_themes(&self) {
        for f in self.theme_listeners.borrow().iter() {
            f();
        }
    }

    /// Re-reads installed themes, after an install or a removal.
    pub fn reload_themes(&self) {
        *self.themes.borrow_mut() = themes::scan();
        self.notify_themes();
    }

    /// Re-reads every launcher, after a change.
    pub fn reload_apps(&self) {
        *self.apps.borrow_mut() = entries::scan();
        for f in self.app_listeners.borrow().iter() {
            f();
        }
    }

    pub fn current_theme(&self) -> String {
        self.current.borrow().clone()
    }

    pub fn apply_theme(&self, id: &str) {
        if id == self.current_theme() {
            return;
        }
        match settings::apply(id) {
            Ok(()) => {
                let name = self.themes.borrow().iter().find(|t| t.id == id).map(|t| t.name.clone());
                self.toast(&format!("Icon theme set to {}", name.as_deref().unwrap_or(id)));
            }
            Err(e) => self.error(&e),
        }
    }

    pub fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::builder().title(glib_escape(text)).timeout(3).build());
    }

    /// Shows a toast with a button that runs a window action.
    pub fn toast_with_action(&self, text: &str, label: &str, action: &str, target: Option<&gtk::glib::Variant>) {
        let toast = adw::Toast::builder().title(glib_escape(text)).button_label(label).timeout(5).build();
        toast.set_action_name(Some(action));
        toast.set_action_target_value(target);
        self.toasts.add_toast(toast);
    }

    pub fn error(&self, e: &anyhow::Error) {
        let toast = adw::Toast::builder().title(glib_escape(&format!("{e:#}"))).timeout(8).build();
        self.toasts.add_toast(toast);
    }
}

/// Toast titles are Pango markup; file names may hold "&" or "<".
fn glib_escape(text: &str) -> String {
    gtk::glib::markup_escape_text(text).to_string()
}
