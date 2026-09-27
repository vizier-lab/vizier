pub mod boot;
pub mod sandbox;
pub mod user;

pub fn init_workspace(path: String) {
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.exists() {
        let _ = std::fs::create_dir_all(path_buf);
    }
}
