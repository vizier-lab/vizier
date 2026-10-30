#[cfg(feature = "shell")]
fn main() {
    vizier_app::shell::run();
}

#[cfg(not(feature = "shell"))]
fn main() {
    eprintln!("vizier-app was built without the `shell` feature; there is no window to open.");
    std::process::exit(2);
}
