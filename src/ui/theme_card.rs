//! One theme in the Themes grid: sample icons from that theme, its name,
//! and whether it is in use. Icons load when the card first shows.

use crate::core::themes::ThemeInfo;
use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// Each slot lists names themes use for the same thing; the first match wins.
const SAMPLES: [&[&str]; 8] = [
    &["folder", "inode-directory"],
    &["user-home", "folder-home"],
    &["org.gnome.Terminal", "org.gnome.Ptyxis", "utilities-terminal", "terminal"],
    &["firefox", "web-browser", "internet-web-browser"],
    &["org.gnome.Settings", "preferences-system", "preferences-desktop"],
    &["org.gnome.Nautilus", "system-file-manager", "nautilus"],
    &["org.gnome.TextEditor", "accessories-text-editor", "text-editor"],
    &["audio-x-generic", "multimedia-audio-player"],
];
const ICON_SIZE: i32 = 40;

pub struct Card {
    pub child: gtk::FlowBoxChild,
    pub remove: gtk::Button,
}

pub fn build(theme: &ThemeInfo, active: bool) -> Card {
    let grid = gtk::Grid::builder().row_spacing(10).column_spacing(10).halign(gtk::Align::Center).build();
    let images: Vec<gtk::Image> = (0..SAMPLES.len())
        .map(|i| {
            let image = gtk::Image::builder().pixel_size(ICON_SIZE).build();
            grid.attach(&image, (i % 4) as i32, (i / 4) as i32, 1, 1);
            image
        })
        .collect();

    let name = gtk::Label::builder().label(&theme.name).css_classes(["heading"]).ellipsize(gtk::pango::EllipsizeMode::End).xalign(0.0).build();
    let id = gtk::Label::builder().label(&theme.id).css_classes(["caption", "dimmed"]).ellipsize(gtk::pango::EllipsizeMode::End).xalign(0.0).build();
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.append(&name);
    if theme.id != theme.name {
        labels.append(&id);
    }

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    footer.append(&labels);
    if active {
        let badge = gtk::Label::builder().label("In Use").css_classes(["badge", "accent"]).valign(gtk::Align::Center).build();
        footer.append(&badge);
    }
    let remove = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove Theme")
        .css_classes(["flat", "circular", "remove"])
        .valign(gtk::Align::Center)
        .visible(theme.user && !active)
        .build();
    footer.append(&remove);

    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(14).css_classes(["card", "theme-card"]).build();
    content.append(&grid);
    content.append(&footer);
    if active {
        content.add_css_class("active");
    }

    let child = gtk::FlowBoxChild::builder().child(&content).build();
    if !theme.comment.is_empty() {
        child.set_tooltip_text(Some(&theme.comment));
    }
    child.update_property(&[gtk::accessible::Property::Label(&theme.name)]);
    let loaded = Rc::new(Cell::new(false));
    let theme_id = theme.id.clone();
    child.connect_map(move |child| {
        if loaded.replace(true) {
            return;
        }
        let icons = gtk::IconTheme::new();
        icons.set_theme_name(Some(&theme_id));
        for (image, names) in images.iter().zip(SAMPLES) {
            let paintable = icons.lookup_icon(
                names[0],
                &names[1..],
                ICON_SIZE,
                child.scale_factor(),
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            );
            image.set_paintable(Some(&paintable));
        }
    });
    Card { child, remove }
}
