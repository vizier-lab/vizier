pub mod boot;
pub mod user;

use std::{fs, path::PathBuf};

use crate::constant::{AGENT_MD, IDENT_MD};

pub fn init_workspace(path: String) {
    let agent_path = PathBuf::from(format!("{}/AGENT.md", path.clone()));
    let ident_path = PathBuf::from(format!("{}/IDENTITY.md", path.clone()));

    let create_file_if_not_exists = |path: PathBuf, content: &str| {
        if !path.exists() {
            let _ = fs::write(path, content);
        }
    };

    let path = PathBuf::from(&path);

    if !path.exists() {
        let _ = std::fs::create_dir_all(path);
    }

    create_file_if_not_exists(agent_path, AGENT_MD);
    create_file_if_not_exists(ident_path, IDENT_MD);
}
