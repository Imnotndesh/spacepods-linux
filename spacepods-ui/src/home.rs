use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Image, ListBox, ListBoxRow, SelectionMode};
use libadwaita::{
    HeaderBar, NavigationView, NavigationPage, ToolbarView, ToastOverlay,
    NavigationSplitView, Breakpoint, BreakpointCondition, ApplicationWindow,
};
use libadwaita::prelude::*;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tray::TrayHandle;
use crate::context::AppContext;
use crate::pages::anc_page::AncPage;
use crate::pages::eq_page::EqPage;
use crate::pages::special_page::SpecialPage;
use crate::pages::settings_page::SettingsPage;
use crate::pages::find_page::FindPage;
use crate::pages::gaming_page::GamingPage;
use crate::pages::spatial_page::SpatialAudioPage;
use crate::pages::hearing_page::HearingPage;

/// Page descriptor for sidebar navigation.
struct PageDef {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    section: Section,
    widget: gtk4::Widget,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Audio,
    Controls,
    Health,
    Settings,
}

impl Section {
    fn label(&self) -> &'static str {
        match self {
            Section::Audio => "Audio",
            Section::Controls => "Controls",
            Section::Health => "Health",
            Section::Settings => "",
        }
    }
}

/// Main home view.
///
/// Layout follows the GNOME HIG "utility pane" pattern:
/// `AdwToastOverlay` (feedback for every page) wrapping an
/// `AdwNavigationSplitView` (sidebar + content, each a real `AdwNavigationPage`
/// so back-gestures and titles work for free) with an `AdwBreakpoint` that
/// collapses the sidebar into a stack on narrow windows/mobile widths,
/// instead of the old fixed-width overlay box.
pub struct HomeView;

impl HomeView {
    pub fn new(
        window: &ApplicationWindow,
        tray_handle: Rc<Option<TrayHandle>>,
        _client: Arc<Mutex<libspacepods::ipc::SpacePodsClient>>,
    ) -> NavigationView {
        let nav_view = NavigationView::new();

        let toast_overlay = ToastOverlay::new();
        let ctx = AppContext::new(toast_overlay.clone());

        let pages: Vec<PageDef> = vec![
            PageDef {
                id: "anc",
                title: "ANC",
                icon: "org.gnome.Settings-accessibility-hearing-symbolic",
                section: Section::Audio,
                widget: AncPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "eq",
                title: "Equalizer",
                icon: "audio-card-symbolic",
                section: Section::Audio,
                widget: EqPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "spatial",
                title: "3D Audio",
                icon: "audio-speakers-symbolic",
                section: Section::Audio,
                widget: SpatialAudioPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "gestures",
                title: "Gestures",
                icon: "input-touchpad-symbolic",
                section: Section::Controls,
                widget: SpecialPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "gaming",
                title: "Game Mode",
                icon: "input-gaming-symbolic",
                section: Section::Controls,
                widget: GamingPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "find",
                title: "Find Earbuds",
                icon: "find-location-symbolic",
                section: Section::Controls,
                widget: FindPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "hearing",
                title: "Hearing Health",
                icon: "heart-symbolic",
                section: Section::Health,
                widget: HearingPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "settings",
                title: "Settings",
                icon: "settings-symbolic",
                section: Section::Settings,
                widget: SettingsPage::navigation_page(tray_handle.clone()).upcast(),
            },
        ];

        let content_stack = gtk4::Stack::new();
        content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(150);

        for def in &pages {
            content_stack.add_named(&def.widget, Some(def.id));
        }
        let sidebar_list = ListBox::new();
        sidebar_list.set_selection_mode(SelectionMode::Single);
        sidebar_list.add_css_class("navigation-sidebar");
        sidebar_list.set_vexpand(true);

        let mut current_section: Option<Section> = None;
        let mut row_to_id: Vec<(ListBoxRow, &'static str)> = Vec::new();

        for page in &pages {
            let needs_header = match (&current_section, &page.section) {
                (None, _) => true,
                (Some(s), sec) if std::mem::discriminant(s) != std::mem::discriminant(sec) => true,
                _ => false,
            };
            if needs_header && !page.section.label().is_empty() {
                let header_row = ListBoxRow::new();
                header_row.set_selectable(false);
                header_row.set_activatable(false);
                header_row.set_focusable(false);
                let lbl = Label::new(Some(page.section.label()));
                lbl.add_css_class("caption-heading");
                lbl.add_css_class("dim-label");
                lbl.set_halign(gtk4::Align::Start);
                lbl.set_margin_top(if current_section.is_some() { 12 } else { 4 });
                lbl.set_margin_bottom(2);
                lbl.set_margin_start(8);
                header_row.set_child(Some(&lbl));
                sidebar_list.append(&header_row);
                current_section = Some(page.section);
            }

            let row_box = Box::new(Orientation::Horizontal, 10);
            row_box.set_margin_top(6);
            row_box.set_margin_bottom(6);
            row_box.set_margin_start(6);
            row_box.set_margin_end(6);

            let icon = Image::from_icon_name(page.icon);
            icon.set_pixel_size(16);
            row_box.append(&icon);

            let lbl = Label::new(Some(page.title));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_hexpand(true);
            row_box.append(&lbl);

            let row = ListBoxRow::new();
            row.set_child(Some(&row_box));
            sidebar_list.append(&row);
            row_to_id.push((row, page.id));
        }

        // Select the first real (non-header) row by default.
        if let Some((first_row, first_id)) = row_to_id.first() {
            sidebar_list.select_row(Some(first_row));
            content_stack.set_visible_child_name(first_id);
        }

        // ── Wire selection -> content stack, and auto-collapse sidebar
        // back to content when a row is picked while collapsed (phone-style
        // navigation, matches AdwNavigationSplitView expectations) ──
        let split_view = NavigationSplitView::new();
        {
            let stack = content_stack.clone();
            let row_to_id = row_to_id.clone();
            let split_view_weak = split_view.downgrade();
            sidebar_list.connect_row_selected(move |_, row| {
                let Some(row) = row else { return };
                if let Some((_, id)) = row_to_id.iter().find(|(r, _)| r == row) {
                    stack.set_visible_child_name(id);
                    if let Some(sv) = split_view_weak.upgrade() {
                        if sv.is_collapsed() {
                            sv.set_show_content(true);
                        }
                    }
                }
            });
        }

        // ── Sidebar header (app identity) ──
        let sidebar_header_box = Box::new(Orientation::Horizontal, 8);
        sidebar_header_box.set_margin_top(4);
        sidebar_header_box.set_margin_bottom(4);
        sidebar_header_box.set_margin_start(8);
        sidebar_header_box.set_margin_end(8);
        let app_icon = Image::from_icon_name("audio-headset-symbolic");
        app_icon.set_pixel_size(20);
        sidebar_header_box.append(&app_icon);
        let device_name = Label::new(Some("SpaceBuds"));
        device_name.add_css_class("heading");
        device_name.set_halign(gtk4::Align::Start);
        device_name.set_hexpand(true);
        sidebar_header_box.append(&device_name);
        let status_dot = Image::from_icon_name("bluetooth-active-symbolic");
        status_dot.set_pixel_size(14);
        status_dot.add_css_class("success");
        status_dot.set_tooltip_text(Some("Connected"));
        sidebar_header_box.append(&status_dot);

        let sidebar_header = HeaderBar::builder()
            .show_title(false)
            .build();
        let sidebar_title = libadwaita::WindowTitle::new("SpacePods", "");
        sidebar_header.set_title_widget(Some(&sidebar_title));

        let sidebar_scroll = gtk4::ScrolledWindow::new();
        sidebar_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        sidebar_scroll.set_vexpand(true);
        sidebar_scroll.set_child(Some(&sidebar_list));

        let sidebar_content = Box::new(Orientation::Vertical, 0);
        sidebar_content.append(&sidebar_header_box);
        sidebar_content.append(&gtk4::Separator::new(Orientation::Horizontal));
        sidebar_content.append(&sidebar_scroll);

        let sidebar_toolbar = ToolbarView::new();
        sidebar_toolbar.add_top_bar(&sidebar_header);
        sidebar_toolbar.set_content(Some(&sidebar_content));

        let sidebar_page = NavigationPage::new(&sidebar_toolbar, "Sidebar");
        sidebar_page.set_width_request(220);

        // ── Content side ──
        let content_header = HeaderBar::new();
        let content_toolbar = ToolbarView::new();
        content_toolbar.add_top_bar(&content_header);
        content_toolbar.set_content(Some(&content_stack));

        let content_page = NavigationPage::new(&content_toolbar, "SpacePods");

        split_view.set_sidebar(Some(&sidebar_page));
        split_view.set_content(Some(&content_page));
        split_view.set_min_sidebar_width(220.0);
        split_view.set_max_sidebar_width(300.0);
        split_view.set_sidebar_width_fraction(0.28);

        toast_overlay.set_child(Some(&split_view));

        // ── Reactive breakpoint: below 680px, collapse to a single pane
        // (GNOME HIG adaptive behaviour — same pattern as Settings/Files) ──
        let condition = BreakpointCondition::new_length(
            libadwaita::BreakpointConditionLengthType::MaxWidth,
            680.0,
            libadwaita::LengthUnit::Px,
        );
        let breakpoint = Breakpoint::new(condition);
        breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(breakpoint);

        let main_page = NavigationPage::builder()
            .title("SpacePods")
            .child(&toast_overlay)
            .build();
        nav_view.push(&main_page);
        nav_view
    }
}