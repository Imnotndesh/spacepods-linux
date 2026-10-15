mod app;
mod autostart;
mod prefs;
mod errors;
mod memstats;
mod widgets;
mod notify;
mod home;
mod log;
mod pages;
mod service;
mod storage;
mod context;
mod tray;
mod quick_settings;

#[cfg(feature = "heap-profile")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Pick the GTK renderer before GTK starts. GTK reads `GSK_RENDERER` once, at startup.
///
/// Order: an explicit `GSK_RENDERER` wins; then `SPACEPODS_RENDERER` (for testing); then the
/// "renderer" preference. The default is Cairo, which uses the least memory (see
/// docs/audit/05-progress.md). Vulkan and OpenGL use the GPU and fall back to other
/// renderers if the driver is missing.
fn choose_renderer() {
    if std::env::var_os("GSK_RENDERER").is_some() {
        return;
    }
    let chosen = std::env::var("SPACEPODS_RENDERER")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| Some(prefs::get_string(prefs::RENDERER)).filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "cairo".to_string());
    // SAFETY: called first thing in main, before any other thread exists.
    unsafe { std::env::set_var("GSK_RENDERER", chosen) };
}

fn main() -> glib::ExitCode {
    choose_renderer();
    // Heap profile: written to dhat-heap.json when main returns. Open it in dh_view.
    #[cfg(feature = "heap-profile")]
    let _profiler = dhat::Profiler::new_heap();
    gio::resources_register_include!("spacepods.gresource")
        .expect("Failed to register resources");

    log::Log::set_level(log::Level::Full);
    memstats::start_if_enabled();
    libspacepods::crash::install_panic_hook("ui");
    app::run_app()
}
