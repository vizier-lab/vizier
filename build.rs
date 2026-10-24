use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
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
             The WebUI and its component library are git submodules: run \
             'git submodule update --init --recursive' and then 'just install', \
             or ensure pre-built files are included."
        );
    }
    // If webui_build_dir exists but node_modules doesn't, use pre-built files (crates.io case)

    // Rebuild if frontend files change
    println!("cargo:rerun-if-changed=webui/app");
    println!("cargo:rerun-if-changed=webui/package.json");
    println!("cargo:rerun-if-changed=design/src");

    embed_catalogue_seed();
    validate_media_seed();
}

/// Gzip the committed model catalogue seed into `$OUT_DIR/catalogue.json.gz`, which
/// `src/catalogue/seed.rs` embeds. Never touches the network: the seed is refreshed by
/// `just catalogue-update` (and once per release), not by the build.
fn embed_catalogue_seed() {
    println!("cargo:rerun-if-changed=assets/catalogue");

    let seed = Path::new("assets/catalogue/api.json");
    let hint = "run `just catalogue-update` to fetch the model catalogue seed";
    let bytes = std::fs::read(seed)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}; {hint}", seed.display()));
    if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        panic!("{} is not valid JSON ({e}); {hint}", seed.display());
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let out = Path::new(&out_dir).join("catalogue.json.gz");
    let file = std::fs::File::create(&out)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", out.display()));
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    encoder
        .write_all(&bytes)
        .and_then(|_| encoder.finish().map(|_| ()))
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
}

/// Validate the committed OpenRouter media seed (spec 028, research R8). It is embedded as-is by
/// `src/catalogue/seed.rs` (small enough not to gzip), so only its syntax is checked here: a
/// malformed file should fail the build early, while a logically empty one is still tolerated
/// at runtime.
fn validate_media_seed() {
    println!("cargo:rerun-if-changed=assets/catalogue/openrouter_media.json");

    let seed = Path::new("assets/catalogue/openrouter_media.json");
    let hint = "run `just catalogue-update` to fetch the OpenRouter media model lists";
    let bytes = std::fs::read(seed)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}; {hint}", seed.display()));
    if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        panic!("{} is not valid JSON ({e}); {hint}", seed.display());
    }
}
