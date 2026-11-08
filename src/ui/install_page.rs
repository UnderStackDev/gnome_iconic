//! The Install page: drop theme archives or folders on the window, or pick
//! them. Work runs off the UI thread; existing themes ask what to do.

use super::ctx::Ctx;
use crate::core::install::{self, Conflict, Prepared};
use adw::prelude::*;
use gtk::{gio, glib};
use std::path::PathBuf;
use std::rc::Rc;

const DESCRIPTION: &str = "Drop an icon theme archive or folder anywhere on this window.\n\
Themes are installed for you only, and Flatpak apps can use them too.";

pub struct InstallPage {
    pub root: adw::StatusPage,
    choose: gtk::Button,
    ctx: Rc<Ctx>,
}

impl InstallPage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let choose = gtk::Button::builder()
            .label("Choose Files…")
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let root = adw::StatusPage::builder()
            .icon_name("folder-download-symbolic")
            .title("Install Icon Themes")
            .description(DESCRIPTION)
            .child(&choose)
            .vexpand(true)
            .build();
        let page = Rc::new(InstallPage { root, choose, ctx: ctx.clone() });
        let weak = Rc::downgrade(&page);
        page.choose.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.pick_files();
            }
        });
        page
    }

    pub fn pick_files(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Icon theme archives"));
        for pattern in ["*.tar", "*.tar.*", "*.tgz", "*.txz", "*.tbz", "*.tbz2", "*.zip", "*.7z"] {
            filter.add_pattern(pattern);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder().title("Choose Icon Themes").filters(&filters).modal(true).build();
        let page = self.clone();
        dialog.open_multiple(Some(&self.ctx.window), gio::Cancellable::NONE, move |result| {
            // Cancel is a normal answer, not an error.
            let Ok(files) = result else { return };
            let paths = files.iter::<gio::File>().flatten().filter_map(|f| f.path()).collect();
            page.install(paths);
        });
    }

    fn set_busy(&self, busy: bool) {
        self.choose.set_sensitive(!busy);
        if busy {
            self.root.set_paintable(Some(&adw::SpinnerPaintable::new(Some(&self.root))));
            self.root.set_description(Some("Unpacking and checking the files…"));
        } else {
            self.root.set_icon_name(Some("folder-download-symbolic"));
            self.root.set_description(Some(DESCRIPTION));
        }
    }

    /// Installs dropped or picked paths (files or folders).
    pub fn install(self: &Rc<Self>, paths: Vec<PathBuf>) {
        if paths.is_empty() || !self.choose.is_sensitive() {
            return;
        }
        self.set_busy(true);
        let page = self.clone();
        glib::spawn_future_local(async move {
            let target = install::default_target();
            let prepared = gio::spawn_blocking(move || {
                install::cleanup_stale(&target)?;
                paths.iter().map(|p| install::prepare(p, &target)).collect::<anyhow::Result<Vec<Prepared>>>()
            })
            .await
            .expect("install thread panicked");
            match prepared {
                Ok(prepared) if prepared.iter().any(Prepared::has_conflicts) => page.ask_conflict(prepared),
                Ok(prepared) => page.finish(prepared, Conflict::Skip),
                Err(e) => {
                    page.set_busy(false);
                    page.ctx.error(&e);
                }
            }
        });
    }

    fn ask_conflict(self: &Rc<Self>, prepared: Vec<Prepared>) {
        let names: Vec<String> =
            prepared.iter().flat_map(|p| &p.themes).filter(|t| t.exists).map(|t| t.name.clone()).collect();
        let dialog = adw::AlertDialog::new(
            Some("Already Installed"),
            Some(&format!("{} is already installed. Replace it, or keep both?", names.join(", "))),
        );
        dialog.add_responses(&[("skip", "Skip"), ("keep", "Keep Both"), ("replace", "Replace")]);
        dialog.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("keep"));
        dialog.set_close_response("skip");
        let page = self.clone();
        let prepared = std::cell::RefCell::new(Some(prepared));
        dialog.connect_response(None, move |_, response| {
            let conflict = match response {
                "replace" => Conflict::Replace,
                "keep" => Conflict::KeepBoth,
                _ => Conflict::Skip,
            };
            if let Some(prepared) = prepared.take() {
                page.finish(prepared, conflict);
            }
        });
        dialog.present(Some(&self.ctx.window));
    }

    fn finish(self: &Rc<Self>, prepared: Vec<Prepared>, conflict: Conflict) {
        let page = self.clone();
        glib::spawn_future_local(async move {
            let target = install::default_target();
            let result = gio::spawn_blocking(move || {
                let (mut ids, mut notes) = (vec![], vec![]);
                for p in prepared {
                    match install::install(p, conflict, &target) {
                        Ok((i, n)) => {
                            ids.extend(i);
                            notes.extend(n);
                        }
                        // Skipped duplicates are notes, not failures.
                        Err(e) if conflict == Conflict::Skip => notes.push(format!("{e:#}")),
                        Err(e) => return Err(e),
                    }
                }
                Ok((ids, notes))
            })
            .await
            .expect("install thread panicked");
            page.set_busy(false);
            match result {
                Ok((ids, notes)) => page.report(&ids, &notes),
                Err(e) => page.ctx.error(&e),
            }
        });
    }

    fn report(&self, ids: &[String], notes: &[String]) {
        self.ctx.reload_themes();
        for note in notes {
            self.ctx.toast(note);
        }
        if let Some(first) = ids.first() {
            let text = format!("Installed {}", ids.join(", "));
            self.ctx.toast_with_action(&text, "Use", "win.apply-theme", Some(&first.to_variant()));
        }
    }
}
