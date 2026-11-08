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

    let label = |text: &str, classes: &[&str]| {
        gtk::Label::builder().label(text).css_classes(classes).ellipsize(gtk::pango::EllipsizeMode::End).justify(gtk::Justification::Center).build()
    };
    let labels = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).halign(gtk::Align::Center).build();
    labels.append(&label(&theme.name, &["heading"]));
    if theme.id != theme.name {
        labels.append(&label(&theme.id, &["caption", "dimmed"]));
    }
    if active {
        let badge = gtk::Label::builder().label("In Use").css_classes(["badge", "accent"]).halign(gtk::Align::Center).margin_top(4).build();
        labels.append(&badge);
    }

    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(14).css_classes(["card", "theme-card"]).build();
    content.append(&grid);
    content.append(&labels);
    if active {
        content.add_css_class("active");
    }
    // In the corner, so the name stays centred under the icons.
    let remove = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Remove Theme")
        .css_classes(["flat", "circular", "remove"])
        .halign(gtk::Align::End)
        .valign(gtk::Align::Start)
        .margin_top(6)
        .margin_end(6)
        .visible(theme.user && !active)
        .build();
    let overlay = gtk::Overlay::builder().child(&content).build();
    overlay.add_overlay(&remove);

    let child = gtk::FlowBoxChild::builder().child(&overlay).build();
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
