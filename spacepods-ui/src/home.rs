use gtk4::prelude::*;
use gtk4::gio;
use gtk4::gio::prelude::ActionMapExt;
use gtk4::{Orientation};
use std::rc::Rc;
use libadwaita::{
    HeaderBar, ToolbarView, ToastOverlay, NavigationPage, NavigationSplitView,
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
use crate::pages::area_tap_page::AreaTapPage;
use crate::pages::hearing_page::HearingPage;
use crate::pages::led_page::LedPage;

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
    /// Build the home view from a pre-constructed AppContext (the caller
    /// already set tray, window, product_id etc.).
    pub fn from_ctx(
        window: &ApplicationWindow,
        ctx: Rc<AppContext>,
    ) -> ToastOverlay {

        Log::info("HOME", &format!("product_id={:?}, profile={:?}", ctx.product_id.get(), ctx.profile().map(|p| p.name)));
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
                id: "areatap",
                title: "Area Tap",
                icon: "input-touchpad-symbolic",
                section: Section::Controls,
                feature: Some(libspacepods::device_profile::DetailFeature::AreaTap),
                widget: AreaTapPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "led",
                title: "LED Lights",
                icon: "display-brightness-symbolic",
                section: Section::Controls,
                feature: Some(libspacepods::device_profile::DetailFeature::Led),
                widget: LedPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "gestures",
                title: "Gestures",
                icon: "gesture-swipe-right-symbolic",
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
                icon: "hearing-health-symbolic",
                section: Section::Health,
                feature: Some(libspacepods::device_profile::DetailFeature::HearingCare),
                widget: HearingPage::new(ctx.clone()).upcast(),
            },
            PageDef {
                id: "settings",
                title: "Settings",
                icon: "settings-symbolic",
                section: Section::Settings,
                feature: None,
                widget: SettingsPage::page(ctx.clone(), {
                    let ww = window.downgrade();
                    let tray = ctx.tray.clone();
                    let wc = ctx.window.clone();
                    move || {
                        if let Some(win) = ww.upgrade() {
                            let ww2 = ww.clone();
                            let tray = tray.clone();
                            let wc = wc.clone();
                            let setup = crate::pages::setup_page::SetupPage::new(
                                move |status: Option<libspacepods::ipc::protocol::DeviceStatus>| {
                                    if let Some(win) = ww2.upgrade() {
                                        let ctx = crate::context::AppContext::new(
                                            libadwaita::ToastOverlay::new(),
                                            tray.clone(),
                                            wc.clone(),
                                        );
                                        if let Some(s) = &status {
                                            ctx.product_id.set(s.product_id);
                                            ctx.capabilities.replace(s.capabilities.clone());
                                        }
                                        let home = HomeView::from_ctx(&win, ctx);
                                        win.set_content(Some(&home));
                                    }
                                }
                            );
                            let toast = libadwaita::ToastOverlay::new();
                            toast.set_child(Some(&setup));
                            win.set_content(Some(&toast));
                        }
                    }
                }),
            },
        ];

        let content_stack = gtk4::Stack::new();
        content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(150);

        for def in &pages {
            content_stack.add_named(&def.widget, Some(def.id));
        }
        // Sidebar: AdwSidebar (libadwaita 1.9). One section per group; the settings page
        // sits in its own untitled section at the bottom.
        let sidebar = libadwaita::Sidebar::new();
        // (item, page id, page title), so selection can find its page.
        let mut entries: Vec<(libadwaita::SidebarItem, &'static str, &'static str)> = Vec::new();
        let mut section: Option<libadwaita::SidebarSection> = None;
        let mut current_section: Option<Section> = None;

        for page in &pages {
            if ctx.product_id.get().is_some() {
                if let Some(feature) = &page.feature {
                    let supported = ctx.has_feature(*feature);
                    Log::full("HOME", &format!("Page '{}' needs {:?} → supported={}", page.id, feature, supported));
                    if !supported {
                        Log::warn("HOME", &format!("Hiding page '{}' — device lacks {:?}", page.id, feature));
                        continue;
                    }
                }
            }
            let starts_section = match (&current_section, &page.section) {
                (None, _) => true,
                (Some(s), sec) => std::mem::discriminant(s) != std::mem::discriminant(sec),
            };
            if starts_section {
                let sec = libadwaita::SidebarSection::new();
                let label = page.section.label();
                if !label.is_empty() {
                    sec.set_title(Some(label));
                }
                sidebar.append(sec.clone());
                section = Some(sec);
                current_section = Some(page.section);
            }
            let item = libadwaita::SidebarItem::builder()
                .title(page.title)
                .icon_name(page.icon)
                .build();
            if let Some(sec) = &section {
                sec.append(item.clone());
            }
            entries.push((item, page.id, page.title));
        }

        // ── Sidebar pane ──
        let sidebar_header = HeaderBar::builder()
            .show_title(false)
            .build();
        let sidebar_title = libadwaita::WindowTitle::new("SpacePods Linux", "Earbud Manager");
        sidebar_header.set_title_widget(Some(&sidebar_title));
        sidebar_header.pack_end(&crate::app::primary_menu_button());

        let sidebar_toolbar = ToolbarView::new();
        sidebar_toolbar.add_top_bar(&sidebar_header);
        sidebar_toolbar.set_content(Some(&sidebar));
        let sidebar_page = NavigationPage::with_tag(&sidebar_toolbar, "SpacePods Linux", "sidebar");

        // ── Content pane ──
        let content_scroll = gtk4::ScrolledWindow::new();
        content_scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        content_scroll.set_vexpand(true);
        content_scroll.set_child(Some(&content_stack));
        // The page scrolls within the window; its content must not push the window taller.
        content_scroll.set_propagate_natural_height(false);
        content_scroll.set_min_content_height(0);

        let content_header = HeaderBar::new();
        let content_title = libadwaita::WindowTitle::new("", "");
        content_header.set_title_widget(Some(&content_title));
        content_header.pack_end(&crate::app::primary_menu_button());

        // Refresh lives in the header. It triggers the visible page's own refresh button,
        // and is shown only for pages that have one.
        let refresh_button = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some("Refresh"));
        refresh_button.update_property(&[gtk4::accessible::Property::Label("Refresh")]);
        {
            let stack = content_stack.clone();
            refresh_button.connect_clicked(move |_| {
                if let Some(child) = stack.visible_child() {
                    if let Some(btn) = find_named(&child, PAGE_REFRESH).and_then(|w| w.downcast::<gtk4::Button>().ok()) {
                        btn.emit_clicked();
                    }
                }
            });
        }
        content_header.pack_end(&refresh_button);

        let content_toolbar = ToolbarView::new();
        content_toolbar.add_top_bar(&content_header);
        content_toolbar.set_content(Some(&content_scroll));
        let content_page = NavigationPage::with_tag(&content_toolbar, "", "content");

        // ── Adaptive split view (collapses into an AdwNavigationView) ──
        let split_view = NavigationSplitView::new();
        split_view.set_min_sidebar_width(220.0);
        split_view.set_max_sidebar_width(280.0);
        split_view.set_sidebar_width_fraction(0.28);
        split_view.set_sidebar(Some(&sidebar_page));
        split_view.set_content(Some(&content_page));

        // Show a page: stack, titles, scroll position, and the collapsed split view.
        let show_page = {
            let refresh_button = refresh_button.clone();
            let stack = content_stack.clone();
            let content_scroll = content_scroll.clone();
            let content_page = content_page.clone();
            let content_title = content_title.clone();
            let split_view_weak = split_view.downgrade();
            Rc::new(move |id: &str, title: &str| {
                stack.set_visible_child_name(id);
                let has_refresh = stack
                    .child_by_name(id)
                    .and_then(|child| find_named(&child, PAGE_REFRESH))
                    .is_some();
                refresh_button.set_visible(has_refresh);
                content_title.set_title(title);
                content_page.set_title(title);
                content_scroll.vadjustment().set_value(0.0);
                let id_owned = id.to_string();
                let stack_weak = stack.downgrade();
                glib::idle_add_local_once(move || {
                    if let Some(s) = stack_weak.upgrade() {
                        if let Some(child) = s.child_by_name(&id_owned) {
                            reset_scroll(&child);
                        }
                    }
                });
                if let Some(sv) = split_view_weak.upgrade() {
                    if sv.is_collapsed() {
                        sv.set_show_content(true);
                    }
                }
            })
        };

        // Select a page by id: moves the sidebar highlight and shows the page.
        let select_page: Rc<dyn Fn(&str)> = {
            let sidebar = sidebar.clone();
            let entries = entries.clone();
            let show_page = show_page.clone();
            Rc::new(move |id: &str| {
                let Some((item, _, title)) = entries.iter().find(|(_, pid, _)| *pid == id) else { return };
                let model = sidebar.items();
                if let Some(index) = (0..model.n_items()).find(|&i| model.item(i).as_ref() == Some(item.upcast_ref())) {
                    sidebar.set_selected(index);
                }
                show_page(id, title);
            })
        };

        // Development aid: SPACEPODS_DEV_PAGE=<page id> opens another page first.
        let initial_id = std::env::var("SPACEPODS_DEV_PAGE")
            .ok()
            .filter(|id| entries.iter().any(|(_, pid, _)| pid == id))
            .or_else(|| entries.first().map(|(_, pid, _)| pid.to_string()));
        if let Some(id) = &initial_id {
            select_page(id);
        }

        // Development aid: SPACEPODS_DEV_SHOW_CONTENT=1 shows the page pane in a collapsed
        // (narrow) window, so narrow layouts can be checked.
        if std::env::var_os("SPACEPODS_DEV_SHOW_CONTENT").is_some() {
            let split_view = split_view.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                split_view.set_show_content(true);
            });
        }

        // Development aid: SPACEPODS_DEV_TOUR=anc,eq,... visits each page in turn (every
        // SPACEPODS_DEV_TOUR_MS milliseconds, default 1500), so memory can be sampled per page.
        if let Ok(tour) = std::env::var("SPACEPODS_DEV_TOUR") {
            let pages: Vec<String> = tour.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
            let interval = std::env::var("SPACEPODS_DEV_TOUR_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
            let select_page = select_page.clone();
            let quit_target = window.clone();
            let mut next = 0usize;
            glib::timeout_add_local(std::time::Duration::from_millis(interval), move || {
                let Some(id) = pages.get(next) else {
                    Log::info("TOUR", "done");
                    // SPACEPODS_DEV_TOUR_QUIT=1 exits after the tour, so a heap profile is written.
                    if std::env::var_os("SPACEPODS_DEV_TOUR_QUIT").is_some() {
                        if let Some(app) = quit_target.application() {
                            app.quit();
                        }
                    }
                    return glib::ControlFlow::Break;
                };
                select_page(id);
                crate::memstats::log_once(&format!("page {}", id));
                next += 1;
                glib::ControlFlow::Continue
            });
        }


        // Clicking a sidebar item shows its page. Programmatic selection goes through
        // `select_page`, so it does not loop back here.
        {
            let entries = entries.clone();
            let show_page = show_page.clone();
            sidebar.connect_activated(move |sb, index| {
                let Some(item) = sb.item(index) else { return };
                if let Some((_, id, title)) = entries.iter().find(|(i, _, _)| *i == item) {
                    show_page(id, title);
                }
            });
        }

        // ── Window actions (accelerators declared in `app::install_app_actions`) ──
        {
            let split_view = split_view.clone();
            let action = gio::SimpleAction::new("toggle-sidebar", None);
            action.connect_activate(move |_, _| {
                if split_view.is_collapsed() {
                    split_view.set_show_content(!split_view.shows_content());
                }
            });
            window.add_action(&action);
        }
        {
            let select_page = select_page.clone();
            let action = gio::SimpleAction::new("show-settings", None);
            action.connect_activate(move |_, _| select_page("settings"));
            window.add_action(&action);
        }

        // Persistent state (service down, earbuds disconnected) is a banner above the
        // content. Toasts stay for one-off feedback.
        let banner = libadwaita::Banner::new("");
        {
            let button_ctx = ctx.clone();
            banner.connect_button_clicked(move |_| {
                match button_ctx.banner_message().map(|(_, action)| action) {
                    Some(crate::context::BannerAction::OpenBluetooth) => open_bluetooth_settings(),
                    _ => button_ctx.check_daemon(),
                }
            });
        }
        let refresh_banner = {
            let banner = banner.clone();
            let ctx = ctx.clone();
            move || match ctx.banner_message() {
                Some((text, action)) => {
                    use crate::context::BannerAction;
                    banner.set_title(text);
                    banner.set_button_label(match action {
                        BannerAction::Retry => Some("Retry"),
                        BannerAction::OpenBluetooth => Some("Open Bluetooth"),
                        BannerAction::None => None,
                    });
                    banner.set_revealed(true);
                }
                None => banner.set_revealed(false),
            }
        };
        refresh_banner();
        ctx.on_daemon_change(refresh_banner);

        let column = gtk4::Box::new(Orientation::Vertical, 0);
        column.append(&banner);
        split_view.set_vexpand(true);
        column.append(&split_view);
        ctx.toast_overlay.set_child(Some(&column));

        let condition = BreakpointCondition::new_length(
            libadwaita::BreakpointConditionLengthType::MaxWidth,
            680.0,
            libadwaita::LengthUnit::Px,
        );
        let breakpoint = Breakpoint::new(condition);
        breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(breakpoint);

        ctx.toast_overlay.clone()
    }
}

/// Recursively find all ScrolledWindows and reset their vertical scroll to top.
fn reset_scroll(widget: &gtk4::Widget) {
    if let Some(sw) = widget.downcast_ref::<gtk4::ScrolledWindow>() {
        sw.vadjustment().set_value(0.0);
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        reset_scroll(&c);
        child = c.next_sibling();
    }
}

/// Widget name of a page's own refresh button (set in each page module).
const PAGE_REFRESH: &str = "page-refresh";

/// Depth-first search for a descendant with the given widget name.
fn find_named(root: &gtk4::Widget, name: &str) -> Option<gtk4::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_named(&c, name) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// Open the Bluetooth settings page. Best effort: outside a Flatpak this runs the
/// settings app directly; if it is missing, the failure is logged.
fn open_bluetooth_settings() {
    if let Err(e) = std::process::Command::new("gnome-control-center").arg("bluetooth").spawn() {
        crate::log::Log::warn("BLUETOOTH", &format!("could not open Bluetooth settings: {}", e));
    }
}
