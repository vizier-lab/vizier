use std::process::Command;

fn main() {
    pyo3_build_config::use_pyo3_cfgs();

    // Run npm build before cargo build
    Command::new("npm")
        .args(&["run", "build"])
        .current_dir("webui")
        .status()
        .expect("Failed to build webui");

    // Rebuild if frontend files change
    println!("cargo:rerun-if-changed=webui/src");
    println!("cargo:rerun-if-changed=webui/package.json");
}
