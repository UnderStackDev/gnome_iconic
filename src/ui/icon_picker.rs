//! Picks an app icon from the current theme: a searchable grid of every
//! app icon name the theme (and its parents) has.

use crate::core::themes;
use adw::prelude::*;
use gtk::{gio, glib};

pub fn open(parent: &impl IsA<gtk::Widget>, theme: &str, on_pick: impl Fn(String) + 'static) {
    let names = gtk::StringList::new(&[]);
    let expression = gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string");
    let filter = gtk::StringFilter::builder()
        .expression(expression)
        .match_mode(gtk::StringFilterMatchMode::Substring)
        .ignore_case(true)
        .build();
    let filtered = gtk::FilterListModel::new(Some(names.clone()), Some(filter.clone()));
    let selection = gtk::SingleSelection::builder().model(&filtered).autoselect(false).build();

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
        let cell = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).margin_top(8).margin_bottom(8).build();
        cell.append(&gtk::Image::builder().pixel_size(48).build());
        cell.append(&gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::Middle).max_width_chars(14).css_classes(["caption"]).build());
        item.set_child(Some(&cell));
    });
    factory.connect_bind(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
        let name = item.item().and_downcast::<gtk::StringObject>().expect("string").string();
        let cell = item.child().expect("cell");
        let image = cell.first_child().and_downcast::<gtk::Image>().expect("image");
        image.set_icon_name(Some(&name));
        let label = image.next_sibling().and_downcast::<gtk::Label>().expect("label");
        label.set_label(&name);
        cell.set_tooltip_text(Some(&name));
    });

    let grid = gtk::GridView::builder().model(&selection).factory(&factory).max_columns(8).single_click_activate(true).build();
    let search = gtk::SearchEntry::builder().placeholder_text("Search icons").margin_start(12).margin_end(12).margin_top(6).margin_bottom(6).build();
    search.connect_search_changed(glib::clone!(#[weak] filter, move |s| filter.set_search(Some(&s.text()))));
    search.set_key_capture_widget(Some(&grid));

    let spinner = adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build();
    let stack = gtk::Stack::new();
    stack.add_named(&spinner, Some("loading"));
    stack.add_named(&gtk::ScrolledWindow::builder().child(&grid).vexpand(true).build(), Some("grid"));

    let header = adw::HeaderBar::new();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&search);
    view.set_content(Some(&stack));
    let dialog = adw::Dialog::builder().title("Pick an Icon").content_width(720).content_height(560).child(&view).build();

    grid.connect_activate(glib::clone!(#[weak] dialog, move |grid, position| {
        let Some(name) = grid.model().and_then(|m| m.item(position)).and_downcast::<gtk::StringObject>() else { return };
        on_pick(name.string().to_string());
        dialog.close();
    }));

    let theme = theme.to_string();
    glib::spawn_future_local(glib::clone!(#[weak] names, #[weak] stack, #[weak] search, async move {
        let list = gio::spawn_blocking(move || themes::app_icon_names(&theme)).await.expect("scan thread panicked");
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        names.splice(0, 0, &refs);
        stack.set_visible_child_name("grid");
        search.grab_focus();
    }));
    dialog.present(Some(parent));
}
