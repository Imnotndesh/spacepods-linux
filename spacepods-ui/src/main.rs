mod app;
mod home;
mod pages;
mod service;
mod storage;
mod tray;
mod context;

fn main() -> glib::ExitCode {
    app::run_app()
}
