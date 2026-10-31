use std::path::{Path, PathBuf};
use std::process::Command;

/// Stages the WebUI that `src/webui/mod.rs` embeds into `$OUT_DIR/webui`.
///
/// In a checkout the source is `webui/build/client`, built here first when `webui/node_modules`
/// exists. In a published `.crate` there is no `webui/`: the publish workflow copies the built
/// files into `webui-dist/` (cargo can't package files outside the crate, and a build script
/// must not write into the source tree).
fn main() {
    let checkout = Path::new("../../webui");
    let packaged = Path::new("webui-dist");

    if checkout.join("node_modules").exists() {
        // On Windows npm is a `.cmd` shim, which `Command` does not find under the bare name.
        // `cfg!` here describes the machine running this script, which is the one running npm.
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let status = Command::new(npm)
            .args(["run", "build"])
            .current_dir(checkout)
            .status()
            .expect("Failed to build webui");

        if !status.success() {
            panic!("Failed to build webui");
        }
    }

    let built = checkout.join("build/client");
    let source: PathBuf = if built.exists() {
        built
    } else if packaged.join("index.html").exists() {
        packaged.to_path_buf()
    } else {
        // No node_modules and no pre-built files: nothing to embed.
        panic!(
            "webui/build/client not found and node_modules not available. \
             The WebUI and its component library are git submodules: run \
             'git submodule update --init --recursive' and then 'just install', \
             or ensure pre-built files are included."
        );
    };

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let dest = Path::new(&out_dir).join("webui");
    let _ = std::fs::remove_dir_all(&dest);
    copy_dir(&source, &dest).unwrap_or_else(|e| {
        panic!("cannot stage {} into {}: {e}", source.display(), dest.display())
    });

    println!("cargo:rerun-if-changed=../../webui/app");
    println!("cargo:rerun-if-changed=../../webui/package.json");
    println!("cargo:rerun-if-changed=../../design/src");
    if source == packaged {
        println!("cargo:rerun-if-changed=webui-dist");
    } else if !checkout.join("node_modules").exists() {
        println!("cargo:rerun-if-changed=../../webui/build/client");
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
