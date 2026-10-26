//! Arcane Dome: a data-driven 2D platform arena shooter.

pub mod ai;
pub mod app;
pub mod data;
pub mod geo;
pub mod platform;
pub mod render;
pub mod sim;

/// Android starts the game here (the APK loads this library and calls it).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn quad_main() {
    platform::install_crash_log();
    macroquad::Window::from_config(app::window_conf(), app::run());
}
