use std::path::Path;
use std::process::Command;

fn main() {
    pyo3_build_config::use_pyo3_cfgs();

    let webui_build_dir = Path::new("webui/build/client");
    let webui_node_modules = Path::new("webui/node_modules");

    // Only run npm build if:
    // 1. node_modules exists (npm install was run)
    // 2. build/client doesn't exist OR we're in development
    if webui_node_modules.exists() {
        // Run npm build before cargo build
        let status = Command::new("npm")
            .args(&["run", "build"])
            .current_dir("webui")
            .status()
            .expect("Failed to build webui");

        if !status.success() {
            panic!("Failed to build webui");
        }
    } else if !webui_build_dir.exists() {
        // If no node_modules and no pre-built files, we can't proceed
        panic!(
            "webui/build/client not found and node_modules not available. \
             Either run 'npm install && npm run build' in webui/ directory, \
             or ensure pre-built files are included."
        );
    }
    // If webui_build_dir exists but node_modules doesn't, use pre-built files (crates.io case)

    // Rebuild if frontend files change
    println!("cargo:rerun-if-changed=webui/app");
    println!("cargo:rerun-if-changed=webui/package.json");
}
