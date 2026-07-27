mod app;
mod home;
mod log;
mod pages;
mod service;
mod storage;
mod context;

fn main() -> glib::ExitCode {
    // Set log level — change to Level::Full for verbose diagnostics
    log::Log::set_level(log::Level::Full);
    app::run_app()
}
