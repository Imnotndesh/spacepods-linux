use gtk4::prelude::*;
use libadwaita::{
    HeaderBar, NavigationView, NavigationPage, ViewStack, ViewSwitcher,
    AboutWindow, ViewSwitcherPolicy, SplitButton, ToolbarView,
};
use gio::{Menu, SimpleActionGroup, SimpleAction};
use glib::clone;
use gtk4::MenuButton;
use std::sync::Arc;
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use crate::pages::anc_page::AncPage;
use crate::pages::eq_page::EqPage;
use crate::storage::load_known_devices;

pub struct HomeView;

impl HomeView {
    pub fn new(
        client: Arc<Mutex<SpacePodsClient>>,
        on_add_device: impl Fn() + 'static + Clone,
    ) -> NavigationView {
        let nav_view = NavigationView::new();

        let view_stack = ViewStack::new();
        view_stack.set_vexpand(true);

        let anc_page = AncPage::new(Arc::clone(&client));
        view_stack.add_titled_with_icon(
            &anc_page, Some("anc"), "ANC",
            "org.gnome.Settings-accessibility-hearing-symbolic",
        );

        let eq_page = EqPage::new(Arc::clone(&client));
        view_stack.add_titled_with_icon(
            &eq_page, Some("eq"), "EQ",
            "audio-card-symbolic",
        );
        view_stack.set_visible_child_name("anc");

        let view_switcher = ViewSwitcher::new();
        view_switcher.set_stack(Some(&view_stack));
        view_switcher.set_policy(ViewSwitcherPolicy::Wide);

        // Populate device menu from saved devices
        let device_menu = Menu::new();
        for device in load_known_devices() {
            device_menu.append(
                Some(&device.name),
                Some(&format!("win.switch-device::{}", device.address)),
            );
        }
        device_menu.append(None, None);
        device_menu.append(Some("Add New Device…"), Some("win.add-device"));

        let split_btn = SplitButton::new();
        split_btn.set_icon_name("list-add-symbolic");
        split_btn.set_tooltip_text(Some("Add new device"));
        split_btn.set_menu_model(Some(&device_menu));
        split_btn.add_css_class("flat");

        let app_menu = Menu::new();
        app_menu.append(Some("Settings"), Some("win.settings"));
        app_menu.append(Some("About"), Some("win.about"));
        let menu_btn = MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_menu_model(Some(&app_menu));

        let header = HeaderBar::new();
        header.set_title_widget(Some(&view_switcher));
        header.pack_start(&split_btn);
        header.pack_end(&menu_btn);

        let actions = SimpleActionGroup::new();

        {
            let on_add = on_add_device.clone();
            let add_action = SimpleAction::new("add-device", None);
            add_action.connect_activate(move |_, _| on_add());
            actions.add_action(&add_action);
        }
        {
            let on_add = on_add_device.clone();
            split_btn.connect_clicked(move |_| on_add());
        }
        {
            let client_ref = Arc::clone(&client);
            let switch_action = SimpleAction::new(
                "switch-device",
                Some(glib::VariantTy::STRING),
            );
            switch_action.connect_activate(move |_, param| {
                if let Some(address) = param.and_then(|p| p.get::<String>()) {
                    let client = Arc::clone(&client_ref);
                    glib::spawn_future_local(async move {
                        let mut c = client.lock().await;
                        if let Err(e) = c.connect_device(address.clone()).await {
                            eprintln!("Failed to switch device to {}: {}", address, e);
                        }
                    });
                }
            });
            actions.add_action(&switch_action);
        }
        {
            let settings_action = SimpleAction::new("settings", None);
            settings_action.connect_activate(clone!(#[weak] nav_view, move |_, _| {
                let page = crate::pages::settings_page::SettingsPage::navigation_page(None);
                nav_view.push(&page);
            }));
            actions.add_action(&settings_action);
        }
        {
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
        }

        nav_view.insert_action_group("win", Some(&actions));
        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&view_stack));
        let main_page = NavigationPage::builder()
            .title("SpacePods")
            .child(&toolbar_view)
            .build();
        nav_view.push(&main_page);
        nav_view
    }
}