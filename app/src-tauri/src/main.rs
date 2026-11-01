// The release build is a GUI program. Without this the linker emits a console-subsystem binary
// and Windows attaches an empty console window to it. A debug build keeps the console, so what
// `cargo tauri dev` prints stays visible. Children then have no console to inherit, which is why
// every spawn in the engine is started with `CREATE_NO_WINDOW` (`vizier_core::process`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "shell")]
fn main() {
    vizier_app::shell::run();
}

#[cfg(not(feature = "shell"))]
fn main() {
    eprintln!("vizier-app was built without the `shell` feature; there is no window to open.");
    std::process::exit(2);
}
