pub mod boot;
pub mod user;

use std::{fs, path::PathBuf};

use crate::{
    constant::{SOUL_MD, IDENT_MD},
    utils::build_path,
};

pub fn init_workspace(path: String) {
    let agent_path = build_path(&path, &["SOUL.md"]);
    let ident_path = build_path(&path, &["IDENTITY.md"]);
    let heartbeat_path = build_path(&path, &["HEARTBEAT.md"]);

    let create_file_if_not_exists = |path: PathBuf, content: &str| {
        if !path.exists() {
            let _ = fs::write(path, content);
        }
    };

    let path_buf = PathBuf::from(&path);

    if !path_buf.exists() {
        let _ = std::fs::create_dir_all(path_buf);
    }

    create_file_if_not_exists(agent_path, SOUL_MD);
    create_file_if_not_exists(ident_path, IDENT_MD);
    create_file_if_not_exists(heartbeat_path, "".into());
}
