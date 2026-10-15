//! Small widgets shared by the pages.

use gtk4::prelude::*;
use gtk4::{Align, Box, Image, Label, Orientation};

/// An icon, a title and a sentence, centred both ways, for empty or error states.
///
/// Built from plain labels rather than an `AdwStatusPage`. Its natural height is small,
/// so it stays centred and fully visible at every window size, including the minimum.
pub fn centered_message(icon: &str, title: &str, description: &str) -> Box {
    let column = Box::new(Orientation::Vertical, 8);
    column.set_halign(Align::Center);
    column.set_valign(Align::Center);
    column.set_hexpand(true);
    column.set_vexpand(true);
    column.set_margin_start(24);
    column.set_margin_end(24);
    column.set_margin_top(24);
    column.set_margin_bottom(24);

    let image = Image::from_icon_name(icon);
    image.set_pixel_size(64);
    image.add_css_class("dim-label");
    column.append(&image);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("title-3");
    title_label.set_justify(gtk4::Justification::Center);
    title_label.set_wrap(true);
    title_label.set_max_width_chars(28);
    column.append(&title_label);

    let description_label = Label::new(Some(description));
    description_label.add_css_class("dim-label");
    description_label.set_justify(gtk4::Justification::Center);
    description_label.set_wrap(true);
    description_label.set_max_width_chars(36);
    column.append(&description_label);

    column
}

/// Resource prefix for the device drawings in `resources/devices/`.
const DEVICES: &str = "/com/github/imnotndesh/spacepods-linux/devices";

/// A device drawing bundled with the app (for example "earbud" or "earbud-mic"), as a
/// paintable for `AdwStatusPage::set_paintable` or `gtk4::Image`. `None` if the resource
/// is missing, so callers can fall back to a stock icon.
pub fn device_paintable(name: &str) -> Option<gtk4::gdk::Paintable> {
    let image = Image::from_resource(&format!("{}/{}.svg", DEVICES, name));
    image.paintable()
}

/// The app's own icon, bundled with the app, for hero spots such as the setup page.
pub fn app_icon_paintable() -> Option<gtk4::gdk::Paintable> {
    let image = Image::from_resource(
        "/com/spacepods/ui/icons/scalable/actions/com.github.imnotndesh.spacepods-linux.svg",
    );
    image.paintable()
}

/// The drawing for a device, chosen from its name: earbuds (in their case), headphones,
/// or a neckband. `None` if the drawing is missing.
pub fn device_art_for_name(name: &str) -> Option<gtk4::gdk::Paintable> {
    use libspacepods::device_profile::{form_for_name, FormFactor};
    let art = match form_for_name(name) {
        FormFactor::Headphones => "headphones",
        FormFactor::Neckband => "neckband",
        FormFactor::InEar | FormFactor::HalfInEar | FormFactor::OpenEar => "earbuds-case",
    };
    device_paintable(art)
}

/// Plays a sequence of drawings in turn, such as the frames under `devices/anim/`.
///
/// Frames are loaded once, up front, and a glib timer swaps them, so nothing waits on the
/// main loop. The timer stops when the image is dropped. When `looping` is false, the
/// sequence ends on `finish` (the still drawing for the device).
pub fn play_frames(
    image: &gtk4::Image,
    prefix: &str,
    count: usize,
    interval_ms: u64,
    looping: bool,
    finish: Option<gtk4::gdk::Paintable>,
) {
    let frames: Vec<gtk4::gdk::Paintable> = (0..count)
        .filter_map(|i| {
            Image::from_resource(&format!("{}/{}{:02}.svg", DEVICES, prefix, i)).paintable()
        })
        .collect();
    if frames.is_empty() {
        return;
    }
    image.set_paintable(Some(&frames[0]));
    let image = image.downgrade();
    let mut next = 1usize;
    glib::timeout_add_local(std::time::Duration::from_millis(interval_ms), move || {
        let Some(image) = image.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if next >= frames.len() {
            if !looping {
                if let Some(still) = &finish {
                    image.set_paintable(Some(still));
                }
                return glib::ControlFlow::Break;
            }
            next = 0;
        }
        image.set_paintable(Some(&frames[next]));
        next += 1;
        glib::ControlFlow::Continue
    });
}

/// Set once the connect animation has played in this run of the app. Reset only by a full
/// restart, so the animation is not repeated while the app is open.
static INTRO_PLAYED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Claim the once-per-run connect animation. Returns true only the first time.
pub fn claim_intro() -> bool {
    !INTRO_PLAYED.swap(true, std::sync::atomic::Ordering::Relaxed)
}

/// The kind of device, from its name, for picking the drawing and animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Earbuds,
    Headphones,
    Neckband,
}

pub fn kind_for_name(name: &str) -> DeviceKind {
    use libspacepods::device_profile::{form_for_name, FormFactor};
    match form_for_name(name) {
        FormFactor::Headphones => DeviceKind::Headphones,
        FormFactor::Neckband => DeviceKind::Neckband,
        FormFactor::InEar | FormFactor::HalfInEar | FormFactor::OpenEar => DeviceKind::Earbuds,
    }
}

/// The still drawing for a device kind.
pub fn still_for_kind(kind: DeviceKind) -> Option<gtk4::gdk::Paintable> {
    device_paintable(match kind {
        DeviceKind::Earbuds => "earbuds-case",
        DeviceKind::Headphones => "headphones",
        DeviceKind::Neckband => "neckband",
    })
}

/// Play the connect animation for a device kind, once, ending on its still drawing.
pub fn play_connect_animation(image: &gtk4::Image, kind: DeviceKind) {
    match kind {
        // Earbuds rise out of the case with an ease-in: 12 frames, about 0.8 s.
        DeviceKind::Earbuds => play_frames(image, "anim/earbuds-out-", 12, 70, false, still_for_kind(kind)),
        // Headphones and neckbands turn once: 24 frames, about 1 s.
        DeviceKind::Headphones => play_frames(image, "anim/headphones-spin-", 24, 40, false, still_for_kind(kind)),
        DeviceKind::Neckband => play_frames(image, "anim/neckband-spin-", 24, 40, false, still_for_kind(kind)),
    }
}

/// The battery drawing for a percentage, in the colour band it falls in. `None` is unknown.
pub fn battery_art(level: Option<u8>) -> Option<gtk4::gdk::Paintable> {
    let name = match level {
        Some(l) if l >= 88 => "battery-full",
        Some(l) if l >= 63 => "battery-three-quarters",
        Some(l) if l >= 38 => "battery-half",
        Some(l) if l >= 13 => "battery-quarter",
        Some(_) => "battery-empty",
        None => "battery-unknown",
    };
    let image = Image::from_resource(&format!("{}/battery/{}.svg", BASE_BATTERY, name));
    image.paintable()
}

/// Resource prefix of the battery drawings.
const BASE_BATTERY: &str = "/com/github/imnotndesh/spacepods-linux";

#[cfg(test)]
mod tests {
    use super::*;

    // The resource manifest is compiled in, so the test checks the frames the connect
    // animation asks for against what the app actually bundles. No display is needed.
    const MANIFEST: &str = include_str!("../resources/resources.gresource.xml");

    #[test]
    fn devices_map_to_the_expected_kind() {
        assert_eq!(kind_for_name("oraimo SpaceBuds Pro"), DeviceKind::Earbuds);
        assert_eq!(kind_for_name("oraimo BoomPop Pro"), DeviceKind::Headphones);
        assert_eq!(kind_for_name("oraimo Neck Sport"), DeviceKind::Neckband);
        // An unknown name falls back to the earbuds drawing, as the ANC page relies on.
        assert_eq!(kind_for_name(""), DeviceKind::Earbuds);
    }

    #[test]
    fn every_animation_frame_is_bundled() {
        // Same frame counts as `play_connect_animation`: 12 for earbuds, 24 for the others.
        for (prefix, frames) in [
            ("anim/earbuds-out-", 12),
            ("anim/headphones-spin-", 24),
            ("anim/neckband-spin-", 24),
        ] {
            for i in 0..frames {
                let alias = format!("alias=\"{}{:02}.svg\"", prefix, i);
                assert!(MANIFEST.contains(&alias), "missing frame {alias}");
            }
            // The frame after the last one must not be needed.
            let extra = format!("alias=\"{}{:02}.svg\"", prefix, frames);
            assert!(!MANIFEST.contains(&extra), "unexpected extra frame {extra}");
        }
    }

    #[test]
    fn every_still_is_bundled() {
        for name in ["earbuds-case", "headphones", "neckband"] {
            let alias = format!("devices/{name}.svg");
            assert!(MANIFEST.contains(&alias), "missing still {alias}");
        }
    }
}
