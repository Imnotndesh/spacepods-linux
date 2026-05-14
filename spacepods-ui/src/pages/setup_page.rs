use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, HeaderBar, PreferencesGroup, StatusPage, ToolbarView, WindowTitle};

pub struct SetupPage;

impl SetupPage {
    pub fn new<F: Fn() + 'static + Clone>(on_complete: F) -> ToolbarView {
        // Header
        let header = HeaderBar::new();
        let title_widget = WindowTitle::new("SpacePods Setup", "Connect your earbuds");
        header.set_title_widget(Some(&title_widget));

        // Close / skip button in header
        let close_btn = gtk4::Button::with_label("Skip");
        close_btn.add_css_class("flat");
        header.pack_end(&close_btn);

        // Status page (hero area)
        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Find Your SpaceBuds");
        status_page.set_description(Some("Make sure your earbuds are in pairing mode and nearby."));
        status_page.set_vexpand(true);

        // Scanning spinner (hidden until scan starts)
        let spinner = Spinner::new();
        spinner.set_size_request(32, 32);
        spinner.set_halign(gtk4::Align::Center);
        spinner.set_visible(false);

        let scan_status = Label::new(None);
        scan_status.add_css_class("dim-label");
        scan_status.set_halign(gtk4::Align::Center);
        scan_status.set_visible(false);

        let spinner_box = Box::new(Orientation::Vertical, 8);
        spinner_box.set_halign(gtk4::Align::Center);
        spinner_box.set_margin_bottom(16);
        spinner_box.append(&spinner);
        spinner_box.append(&scan_status);

        // Preferences group with action rows
        let group = PreferencesGroup::new();

        let scan_row = ActionRow::new();
        scan_row.set_title("Scan for devices");
        scan_row.set_subtitle("Search for nearby SpaceBuds over Bluetooth");
        scan_row.set_activatable(true);
        let scan_chevron = gtk4::Image::from_icon_name("bluetooth-symbolic");
        scan_chevron.add_css_class("dim-label");
        scan_row.add_prefix(&scan_chevron);
        let scan_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        scan_arrow.add_css_class("dim-label");
        scan_row.add_suffix(&scan_arrow);

        group.add(&scan_row);

        // Layout
        let clamp = Clamp::new();
        clamp.set_maximum_size(480);
        clamp.set_tightening_threshold(400);

        let content = Box::new(Orientation::Vertical, 0);
        content.set_margin_top(24);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&status_page);
        content.append(&spinner_box);
        content.append(&group);

        clamp.set_child(Some(&content));

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scroll));

        // Scan row clicked
        {
            let spinner_ref = spinner.clone();
            let scan_status_ref = scan_status.clone();
            let spinner_box_ref = spinner_box.clone();
            let scan_row_ref = scan_row.clone();
            let on_complete_clone = on_complete.clone();

            scan_row.connect_activated(move |_| {
                scan_row_ref.set_sensitive(false);
                spinner_box_ref.set_visible(true);
                spinner_ref.start();
                scan_status_ref.set_text("Scanning for SpaceBuds…");
                scan_status_ref.set_visible(true);

                let on_complete = on_complete_clone.clone();
                glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
                    on_complete();
                    glib::ControlFlow::Break
                });
            });
        }

        // Skip / close button
        close_btn.connect_clicked(move |_| {
            on_complete();
        });

        toolbar_view
    }
}