//! The main window: a header with the page switcher, three pages, toasts,
//! window actions and keys, and dropping theme files anywhere.

use super::{apps_page::AppsPage, ctx::Ctx, dialogs, install_page::InstallPage, themes_page::ThemesPage};
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::rc::Rc;

pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder().application(app).title("Iconic").default_width(1000).default_height(740).width_request(360).height_request(400).build();
    let toasts = adw::ToastOverlay::new();
    let ctx = Ctx::new(window.clone(), toasts.clone());

    let themes = ThemesPage::new(&ctx);
    let install = InstallPage::new(&ctx);
    let apps = AppsPage::new(&ctx);
    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(&themes.root, Some("themes"), "Themes", "preferences-desktop-appearance-symbolic");
    stack.add_titled_with_icon(&install.root, Some("install"), "Install", "folder-download-symbolic");
    stack.add_titled_with_icon(&apps.root, Some("apps"), "Apps", "view-app-grid-symbolic");

    let menu = gio::Menu::new();
    menu.append(Some("_Keyboard Shortcuts"), Some("win.shortcuts"));
    menu.append(Some("_About Iconic"), Some("win.about"));
    let menu_button = gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).primary(true).tooltip_text("Main Menu").build();
    let header = adw::HeaderBar::builder()
        .title_widget(&adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build())
        .build();
    header.pack_end(&menu_button);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    toasts.set_child(Some(&stack));
    view.set_content(Some(&toasts));
    window.set_content(Some(&view));

    add_actions(&ctx, &stack, &themes, &install, &apps);
    add_drop(&window, &stack, &install);
    add_keys(&window);
    stack.connect_visible_child_name_notify(glib::clone!(#[strong] themes, #[strong] apps, move |s| {
        match s.visible_child_name().as_deref() {
            Some("themes") => themes.focus_grid(),
            Some("apps") => apps.focus_list(),
            _ => {}
        }
    }));
    // After the first layout, so the grid can scroll to the theme in use.
    glib::idle_add_local_once(glib::clone!(#[strong] themes, move || themes.focus_grid()));
    #[cfg(debug_assertions)]
    {
        let a = apps.clone();
        action(&window, "debug-edit", move |v| a.select(v.and_then(|v| v.str()).unwrap_or_default()), Some(glib::VariantTy::STRING));
    }
    // Narrow windows: the launcher editor slides over the list.
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 760sp").expect("valid condition"));
    narrow.add_setter(&apps.root, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);
    window
}

fn action(window: &adw::ApplicationWindow, name: &str, f: impl Fn(Option<&glib::Variant>) + 'static, arg: Option<&glib::VariantTy>) {
    let a = gio::SimpleAction::new(name, arg);
    a.connect_activate(move |_, v| f(v));
    window.add_action(&a);
}

fn add_actions(ctx: &Rc<Ctx>, stack: &adw::ViewStack, themes: &Rc<ThemesPage>, install: &Rc<InstallPage>, apps: &Rc<AppsPage>) {
    let w = &ctx.window;
    let c = ctx.clone();
    action(w, "apply-theme", move |v| c.apply_theme(&v.and_then(|v| v.str()).unwrap_or_default().to_string()), Some(glib::VariantTy::STRING));
    let s = stack.clone();
    action(w, "page", move |v| s.set_visible_child_name(v.and_then(|v| v.str()).unwrap_or("themes")), Some(glib::VariantTy::STRING));
    let (s, t, a) = (stack.clone(), themes.clone(), apps.clone());
    action(w, "search", move |_| match s.visible_child_name().as_deref() {
        Some("apps") => { a.search.grab_focus(); }
        _ => {
            s.set_visible_child_name("themes");
            t.search.grab_focus();
        }
    }, None);
    let (s, t) = (stack.clone(), themes.clone());
    action(w, "remove-theme", move |_| if s.visible_child_name().as_deref() == Some("themes") { t.remove_picked() }, None);
    let (s, i) = (stack.clone(), install.clone());
    action(w, "install-files", move |_| {
        s.set_visible_child_name("install");
        i.pick_files();
    }, None);
    let c = ctx.clone();
    action(w, "undo", move |_| history_step(&c, true), None);
    let c = ctx.clone();
    action(w, "redo", move |_| history_step(&c, false), None);
    let win = w.clone();
    action(w, "shortcuts", move |_| dialogs::shortcuts(&win), None);
    let win = w.clone();
    action(w, "about", move |_| dialogs::about(&win), None);
}

fn history_step(ctx: &Rc<Ctx>, undo: bool) {
    let result = if undo { ctx.history.borrow_mut().undo() } else { ctx.history.borrow_mut().redo() };
    match result {
        Ok(Some(id)) => {
            ctx.reload_apps();
            ctx.toast(&format!("{} {id}", if undo { "Undid the change to" } else { "Redid the change to" }));
        }
        Ok(None) => ctx.toast(if undo { "Nothing to undo" } else { "Nothing to redo" }),
        Err(e) => ctx.error(&e),
    }
}

/// Theme archives or folders dropped anywhere go to the Install page,
/// except on the Apps page, where drops are icons for the launcher editor.
fn add_drop(window: &adw::ApplicationWindow, stack: &adw::ViewStack, install: &Rc<InstallPage>) {
    let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    target.connect_accept(glib::clone!(#[weak] stack, #[upgrade_or] false, move |_, _| stack.visible_child_name().as_deref() != Some("apps")));
    target.connect_drop(glib::clone!(#[weak] stack, #[strong] install, #[upgrade_or] false, move |_, value, _, _| {
        let Ok(files) = value.get::<gdk::FileList>() else { return false };
        stack.set_visible_child_name("install");
        install.install(files.files().iter().filter_map(|f| f.path()).collect());
        true
    }));
    window.add_controller(target);
}

/// Single keys work when no text field has the focus (bubble phase).
fn add_keys(window: &adw::ApplicationWindow) {
    let keys = gtk::ShortcutController::new();
    let add = |trigger: &str, action: &str, arg: Option<glib::Variant>| {
        let shortcut = gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(gtk::NamedAction::new(action)));
        shortcut.set_arguments(arg.as_ref());
        keys.add_shortcut(shortcut);
    };
    add("1", "win.page", Some("themes".to_variant()));
    add("2", "win.page", Some("install".to_variant()));
    add("3", "win.page", Some("apps".to_variant()));
    add("<Alt>1", "win.page", Some("themes".to_variant()));
    add("<Alt>2", "win.page", Some("install".to_variant()));
    add("<Alt>3", "win.page", Some("apps".to_variant()));
    add("slash", "win.search", None);
    add("Delete", "win.remove-theme", None);
    window.add_controller(keys);
}
