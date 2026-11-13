//! The command line. Every command takes --json and exits 0 or 1.

use crate::core::install::{self, Conflict};
use crate::core::{entries, launchers, settings, themes};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

const USAGE: &str = "Usage: gnome-iconic [command]

  (no command), open        Open the window
  status                    Show the icon theme
  list-themes               List installed icon themes
  apply <theme>             Set the desktop's icon theme
  install <archive|folder>... [--replace|--keep-both] [--apply]
                            Install icon themes into ~/.local/share/icons
  app list                  List launchers (--modified, --hidden to filter)
  app set <id> [--name <name>] [--icon <name|file>] [--hide|--show]
                            Change a launcher (a copy; the original stays)
  app reset <id>            Undo Iconic's changes to a launcher

Every command takes --json.
";

pub fn wants_cli(args: &[String]) -> bool {
    args.first().is_some_and(|a| a != "open")
}

/// Runs one command; returns the exit code.
pub fn run(mut args: Vec<String>) -> i32 {
    let json = args.iter().any(|a| a == "--json");
    args.retain(|a| a != "--json");
    match dispatch(&args) {
        Ok((value, text)) => {
            if json {
                let mut out = serde_json::Map::from_iter([("ok".to_string(), json!(true))]);
                out.extend(value.as_object().cloned().unwrap_or_default());
                println!("{}", Value::Object(out));
            } else if !text.is_empty() {
                println!("{text}");
            }
            0
        }
        Err(e) => {
            if json {
                println!("{}", json!({"ok": false, "error": format!("{e:#}")}));
            }
            eprintln!("gnome-iconic: {e:#}");
            1
        }
    }
}

fn option(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn apply_theme(id: &str) -> Result<()> {
    if !themes::scan().iter().any(|t| t.id == id) {
        bail!("no icon theme \"{id}\" (see gnome-iconic list-themes)");
    }
    settings::apply(id)
}

fn dispatch(args: &[String]) -> Result<(Value, String)> {
    let command = args.first().map(String::as_str).unwrap_or("");
    match command {
        "help" | "--help" | "-h" => Ok((json!({}), USAGE.trim_end().into())),
        "status" => {
            let theme = settings::current_theme();
            Ok((json!({"theme": theme}), theme))
        }
        "list-themes" => {
            let current = settings::current_theme();
            let list = themes::scan();
            let text = list
                .iter()
                .map(|t| format!("{} {}{}", if t.id == current { "*" } else { " " }, t.id, if t.user { "  (user)" } else { "" }))
                .collect::<Vec<_>>()
                .join("\n");
            let rows: Vec<Value> = list
                .iter()
                .map(|t| json!({"id": t.id, "name": t.name, "user": t.user, "path": t.path, "active": t.id == current}))
                .collect();
            Ok((json!({"themes": rows}), text))
        }
        "apply" => {
            let id = args.get(1).filter(|a| !a.starts_with("--")).context("apply needs a theme name (see gnome-iconic list-themes)")?;
            apply_theme(id)?;
            Ok((json!({"theme": id}), format!("Icon theme: {id}")))
        }
        "install" => install_command(args),
        "app" => app_command(args),
        _ => {
            eprint!("{USAGE}");
            bail!("unknown command: {command}")
        }
    }
}

fn install_command(args: &[String]) -> Result<(Value, String)> {
    let sources: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with('-')).collect();
    if sources.is_empty() {
        bail!("install needs an archive or folder");
    }
    let conflict = if has(args, "--replace") {
        Conflict::Replace
    } else if has(args, "--keep-both") {
        Conflict::KeepBoth
    } else {
        Conflict::Skip
    };
    let target = install::default_target();
    install::cleanup_stale(&target)?;
    let (mut installed, mut notes) = (vec![], vec![]);
    for source in sources {
        let prepared = install::prepare(Path::new(source), &target)?;
        let (ids, n) = install::install(prepared, conflict, &target)?;
        installed.extend(ids);
        notes.extend(n);
    }
    if has(args, "--apply") {
        apply_theme(&installed[0])?;
    }
    let mut text: Vec<String> = installed.iter().map(|id| format!("Installed {id}")).collect();
    text.extend(notes.iter().cloned());
    Ok((json!({"installed": installed, "notes": notes}), text.join("\n")))
}

fn app_command(args: &[String]) -> Result<(Value, String)> {
    let sub = args.get(1).map(String::as_str).unwrap_or("");
    let apps = entries::scan();
    if sub == "list" {
        let shown: Vec<&entries::Entry> = apps
            .iter()
            .filter(|e| !has(args, "--modified") || e.modified())
            .filter(|e| !has(args, "--hidden") || e.hidden)
            .collect();
        let text = shown
            .iter()
            .map(|e| format!("{}{}  {}  ({})", if e.modified() { "* " } else { "  " }, e.id, e.name, e.icon))
            .collect::<Vec<_>>()
            .join("\n");
        let rows: Vec<Value> = shown
            .iter()
            .map(|e| json!({"id": e.id, "name": e.name, "icon": e.icon, "hidden": e.hidden, "modified": e.modified(), "path": e.path}))
            .collect();
        return Ok((json!({"apps": rows}), text));
    }
    let wanted = args.get(2).map(|s| s.trim_end_matches(".desktop")).unwrap_or("");
    let entry = apps.iter().find(|e| e.id == wanted).with_context(|| format!("no launcher with id \"{wanted}\" (see gnome-iconic app list)"))?;
    match sub {
        "set" => {
            let mut icon = option(args, "--icon");
            if let Some(i) = icon.as_ref().filter(|i| Path::new(i).is_file()) {
                let abs = std::fs::canonicalize(i)?;
                icon = Some(launchers::import_icon(&abs, &entry.id)?.to_string_lossy().into_owned());
            }
            let hidden = if has(args, "--hide") { Some(true) } else if has(args, "--show") { Some(false) } else { None };
            launchers::apply(entry, &launchers::Change { name: option(args, "--name"), icon, hidden })?;
            Ok((json!({"id": entry.id}), format!("Changed {}", entry.id)))
        }
        "reset" => {
            launchers::reset(entry)?;
            Ok((json!({"id": entry.id}), format!("Reset {}", entry.id)))
        }
        _ => bail!("app needs list, set or reset"),
    }
}
