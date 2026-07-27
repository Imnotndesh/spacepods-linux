use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Image, ListBox, ListBoxRow, SelectionMode};
use libadwaita::{
    HeaderBar, ToolbarView, ToastOverlay, OverlaySplitView,
    Breakpoint, BreakpointCondition, ApplicationWindow,
};
use libadwaita::prelude::*;
use crate::context::AppContext;
use crate::log::Log;
use crate::pages::anc_page::AncPage;
use crate::pages::eq_page::EqPage;
use crate::pages::special_page::SpecialPage;
use crate::pages::settings_page::SettingsPage;
use crate::pages::gaming_page::GamingPage;
use crate::pages::spatial_page::SpatialAudioPage;
use crate::pages::hearing_page::HearingPage;

/// Page descriptor for sidebar navigation.
struct PageDef {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    section: Section,
    /// If set, this page is only shown when the connected device supports this feature.
    /// None means always visible.
    feature: Option<libspacepods::device_profile::DetailFeature>,
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
/// Layout: `AdwToastOverlay` wrapping an `AdwNavigationSplitView`
/// (sidebar + content). No outer `AdwNavigationView` — the split view
/// itself contains `AdwNavigationPage` children. A breakpoint collapses
/// the sidebar on narrow windows.
pub struct HomeView;

impl HomeView {
    pub fn new(
        window: &ApplicationWindow,
        product_id: Option<u16>,
    ) -> ToastOverlay {
        let toast_overlay = ToastOverlay::new();
        let ctx = AppContext::new(toast_overlay.clone());
        ctx.product_id.set(product_id);

        Log::info("HOME", &format!("product_id={:?}, profile={:?}", product_id, ctx.profile().map(|p| p.name)));
        Log::full("HOME", &format!("features={:?}", ctx.profile().map(|p| &p.features)));

        let pages: Vec<PageDef> = vec![
            PageDef {
                id: "anc",
                title: "ANC",
                icon: "org.gnome.Settings-accessibility-hearing-symbolic",
                section: Section::Audio,
                feature: Some(libspacepods::device_profile::DetailFeature::Noise),
                widget: AncPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "eq",
                title: "Equalizer",
                icon: "audio-card-symbolic",
                section: Section::Audio,
                feature: None, // always show EQ
                widget: EqPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "spatial",
                title: "3D Audio",
                icon: "audio-speakers-symbolic",
                section: Section::Audio,
                feature: Some(libspacepods::device_profile::DetailFeature::SpaceAudio),
                widget: SpatialAudioPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "gestures",
                title: "Gestures",
                icon: "input-touchpad-symbolic",
                section: Section::Controls,
                feature: Some(libspacepods::device_profile::DetailFeature::EarControl),
                widget: SpecialPage::new(ctx.clone()),
            },
            PageDef {
                id: "gaming",
                title: "Game Mode",
                icon: "input-gaming-symbolic",
                section: Section::Controls,
                feature: Some(libspacepods::device_profile::DetailFeature::GameMode),
                widget: GamingPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "hearing",
                title: "Hearing Health",
                icon: "heart-symbolic",
                section: Section::Health,
                feature: Some(libspacepods::device_profile::DetailFeature::HearingCare),
                widget: HearingPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "settings",
                title: "Settings",
                icon: "settings-symbolic",
                section: Section::Settings,
                feature: None, // always show settings
                widget: SettingsPage::page(),
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
            // Filter: skip pages gated by features the device doesn't support
            if let Some(feature) = &page.feature {
                let supported = ctx.has_feature(*feature);
                Log::full("HOME", &format!("Page '{}' needs {:?} → supported={}", page.id, feature, supported));
                if !supported {
                    Log::warn("HOME", &format!("Hiding page '{}' — device lacks {:?}", page.id, feature));
                    continue;
                }
            }
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

        if let Some((first_row, first_id)) = row_to_id.first() {
            sidebar_list.select_row(Some(first_row));
            content_stack.set_visible_child_name(first_id);
        }

        // ── Wire selection -> content stack ──
        let split_view = OverlaySplitView::builder()
            .min_sidebar_width(220.0)
            .max_sidebar_width(280.0)
            .sidebar_width_fraction(0.28)
            .build();
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
                            sv.set_show_sidebar(false);
                        }
                    }
                }
            });
        }

        // ── Sidebar ──
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
        sidebar_content.append(&sidebar_scroll);

        let sidebar_toolbar = ToolbarView::new();
        sidebar_toolbar.add_top_bar(&sidebar_header);
        sidebar_toolbar.set_content(Some(&sidebar_content));

        // ── Content side ──
        let content_header = HeaderBar::new();
        let content_title = libadwaita::WindowTitle::new("", "");
        content_header.set_title_widget(Some(&content_title));
        content_header.set_show_back_button(false);

        let content_scroll = gtk4::ScrolledWindow::new();
        content_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        content_scroll.set_vexpand(true);
        content_scroll.set_child(Some(&content_stack));

        let content_toolbar = ToolbarView::new();
        content_toolbar.add_top_bar(&content_header);
        content_toolbar.set_content(Some(&content_scroll));

        split_view.set_sidebar(Some(&sidebar_toolbar));
        split_view.set_content(Some(&content_toolbar));

        toast_overlay.set_child(Some(&split_view));

        // ── Breakpoint ──
        let condition = BreakpointCondition::new_length(
            libadwaita::BreakpointConditionLengthType::MaxWidth,
            680.0,
            libadwaita::LengthUnit::Px,
        );
        let breakpoint = Breakpoint::new(condition);
        breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(breakpoint);

        toast_overlay
    }
}
