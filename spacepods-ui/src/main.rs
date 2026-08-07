mod app;
mod home;
mod log;
mod pages;
mod service;
mod storage;
mod context;

fn main() -> glib::ExitCode {
    gio::resources_register_include!("spacepods.gresource")
        .expect("Failed to register resources");

    log::Log::set_level(log::Level::Full);
    app::run_app()
}
