use gtk4::prelude::*;
use libadwaita::{HeaderBar, NavigationView, NavigationPage, ViewStack, ViewSwitcher, AboutWindow, ViewSwitcherPolicy};
use gio::{Menu, SimpleActionGroup, SimpleAction};
use glib::clone;
use gtk4::MenuButton;
use crate::pages::anc_page::AncPage;
use crate::pages::eq_page::EqPage;

pub struct HomeView;

impl HomeView {
    pub fn new() -> NavigationView {
        let nav_view = NavigationView::new();

        let view_stack = ViewStack::new();
        view_stack.set_vexpand(true);

        let anc_page = AncPage::new();
        view_stack.add_titled_with_icon(&anc_page,Some("anc"),&String::from("ANC"),&String::from("org.gnome.Settings-accessibility-hearing-symbolic"));

        let eq_page = EqPage::new();
        view_stack.add_titled_with_icon(&eq_page, Some("eq"), &String::from("EQ"),&String::from("audio-card-symbolic"));

        view_stack.set_visible_child_name("anc");

        let view_switcher = ViewSwitcher::new();
        view_switcher.set_stack(Option::from(&view_stack));
        view_switcher.set_policy(ViewSwitcherPolicy::Wide);

        let header = HeaderBar::new();
        header.set_title_widget(Some(&view_switcher));
        let menu_btn = MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");

        let menu = Menu::new();
        menu.append(Some("Settings"), Some("win.settings"));
        menu.append(Some("About"), Some("win.about"));
        menu_btn.set_menu_model(Some(&menu));

        header.pack_end(&menu_btn);

        let actions = SimpleActionGroup::new();

        let settings_action = SimpleAction::new("settings", None);
        settings_action.connect_activate(clone!(#[weak] nav_view, move |_, _| {
            let settings_nav_page = crate::pages::settings_page::SettingsPage::navigation_page(None);
            nav_view.push(&settings_nav_page);
        }));
        actions.add_action(&settings_action);

        let about_action = SimpleAction::new("about", None);
        about_action.connect_activate(move |_, _| {
            let about = AboutWindow::builder()
                .application_name("SpacePods")
                .version(env!("CARGO_PKG_VERSION"))
                .comments("Control your SpaceBuds earbuds")
                .website("https://github.com/Imnotndesh/spacepods-ui")
                .developers(vec!["Brian Njoroge <brian@ndegwa.uk>"])
                .copyright("© 2025 Brian Njoroge")
                .license_type(gtk4::License::Gpl30)
                .build();
            about.present();
        });
        actions.add_action(&about_action);
        nav_view.insert_action_group("win", Some(&actions));

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        main_box.append(&header);
        main_box.append(&view_stack);

        let main_page = NavigationPage::builder()
            .title("SpacePods")
            .child(&main_box)
            .build();
        nav_view.push(&main_page);

        nav_view
    }
}