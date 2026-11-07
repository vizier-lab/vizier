//! The built-in table of languages (`data-model.md` §3). One row per language: which files it
//! owns, which server answers for it, and how its root is found. Nothing branches on a language
//! id: a difference between servers is a field here or a negotiated capability (Principle II).

use std::path::{Path, PathBuf};

/// How a language finds the root a server is started in.
#[derive(Debug, Clone, Copy)]
pub enum RootRule {
    /// The nearest folder (from the file up to the project folder) holding any marker.
    Nearest(&'static [&'static str]),
    /// A `prefer` marker at any level wins (the topmost one); otherwise the nearest `markers` folder.
    NearestPreferring {
        markers: &'static [&'static str],
        prefer: &'static [&'static str],
    },
    /// The nearest ancestor `Cargo.toml` that declares `[workspace]`, else the nearest `Cargo.toml`.
    CargoWorkspace,
}

#[derive(Debug)]
pub struct LanguageDef {
    pub id: &'static str,
    pub name: &'static str,
    /// `(extension, LSP languageId)`.
    pub extensions: &'static [(&'static str, &'static str)],
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub root: RootRule,
    pub install_hint: &'static str,
    /// JSON sent as `initializationOptions`.
    pub initialization_options: Option<&'static str>,
}

impl LanguageDef {
    pub fn lsp_language_id(&self, path: &Path) -> &'static str {
        let ext = extension(path);
        self.extensions
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, id)| *id)
            .unwrap_or(self.id)
    }
}

const fn def(
    id: &'static str,
    name: &'static str,
    extensions: &'static [(&'static str, &'static str)],
    command: &'static str,
    args: &'static [&'static str],
    root: RootRule,
    install_hint: &'static str,
) -> LanguageDef {
    LanguageDef {
        id,
        name,
        extensions,
        command,
        args,
        root,
        install_hint,
        initialization_options: None,
    }
}

pub static LANGUAGES: &[LanguageDef] = &[
    LanguageDef {
        initialization_options: Some(r#"{"checkOnSave": false}"#),
        ..def(
            "rust",
            "Rust",
            &[("rs", "rust")],
            "rust-analyzer",
            &[],
            RootRule::CargoWorkspace,
            "rustup component add rust-analyzer",
        )
    },
    def(
        "typescript",
        "TypeScript/JavaScript",
        &[
            ("ts", "typescript"),
            ("mts", "typescript"),
            ("cts", "typescript"),
            ("tsx", "typescriptreact"),
            ("js", "javascript"),
            ("mjs", "javascript"),
            ("cjs", "javascript"),
            ("jsx", "javascriptreact"),
        ],
        "typescript-language-server",
        &["--stdio"],
        RootRule::Nearest(&["tsconfig.json", "jsconfig.json", "package.json"]),
        "npm install -g typescript typescript-language-server",
    ),
    def(
        "python",
        "Python",
        &[("py", "python"), ("pyi", "python")],
        "pyright-langserver",
        &["--stdio"],
        RootRule::Nearest(&[
            "pyrightconfig.json",
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "requirements.txt",
            "Pipfile",
        ]),
        "npm install -g pyright",
    ),
    def(
        "go",
        "Go",
        &[("go", "go")],
        "gopls",
        &[],
        RootRule::NearestPreferring {
            markers: &["go.mod"],
            prefer: &["go.work"],
        },
        "go install golang.org/x/tools/gopls@latest",
    ),
    def(
        "cpp",
        "C/C++",
        &[
            ("c", "c"),
            ("h", "c"),
            ("cc", "cpp"),
            ("cpp", "cpp"),
            ("cxx", "cpp"),
            ("hh", "cpp"),
            ("hpp", "cpp"),
            ("hxx", "cpp"),
        ],
        "clangd",
        &[],
        RootRule::Nearest(&[
            "compile_commands.json",
            "compile_flags.txt",
            ".clangd",
            "CMakeLists.txt",
        ]),
        "Install LLVM or the Xcode command line tools",
    ),
    def(
        "lua",
        "Lua",
        &[("lua", "lua")],
        "lua-language-server",
        &[],
        RootRule::Nearest(&[".luarc.json", ".luarc.jsonc", ".stylua.toml", "stylua.toml"]),
        "brew install lua-language-server, or see https://luals.github.io",
    ),
    def(
        "java",
        "Java",
        &[("java", "java")],
        "jdtls",
        &[],
        RootRule::NearestPreferring {
            markers: &["pom.xml", "build.gradle", "build.gradle.kts"],
            prefer: &["settings.gradle", "settings.gradle.kts"],
        },
        "brew install jdtls, or download it from https://github.com/eclipse-jdtls/eclipse.jdt.ls",
    ),
    def(
        "kotlin",
        "Kotlin",
        &[("kt", "kotlin"), ("kts", "kotlin")],
        "kotlin-language-server",
        &[],
        RootRule::NearestPreferring {
            markers: &["pom.xml", "build.gradle", "build.gradle.kts"],
            prefer: &["settings.gradle", "settings.gradle.kts"],
        },
        "brew install kotlin-language-server, or see https://github.com/fwcd/kotlin-language-server",
    ),
    def(
        "csharp",
        "C#",
        &[("cs", "csharp")],
        "csharp-ls",
        &[],
        RootRule::NearestPreferring {
            markers: &["*.csproj"],
            prefer: &["*.sln", "*.slnx"],
        },
        "dotnet tool install --global csharp-ls",
    ),
    def(
        "ruby",
        "Ruby",
        &[("rb", "ruby")],
        "ruby-lsp",
        &[],
        RootRule::Nearest(&["Gemfile", ".ruby-version"]),
        "gem install ruby-lsp",
    ),
    def(
        "php",
        "PHP",
        &[("php", "php")],
        "intelephense",
        &["--stdio"],
        RootRule::Nearest(&["composer.json"]),
        "npm install -g intelephense",
    ),
    def(
        "swift",
        "Swift",
        &[("swift", "swift")],
        "sourcekit-lsp",
        &[],
        RootRule::Nearest(&["Package.swift"]),
        "Comes with the Swift toolchain or Xcode",
    ),
    def(
        "dart",
        "Dart",
        &[("dart", "dart")],
        "dart",
        &["language-server"],
        RootRule::Nearest(&["pubspec.yaml"]),
        "Comes with the Dart or Flutter SDK",
    ),
    def(
        "elixir",
        "Elixir",
        &[("ex", "elixir"), ("exs", "elixir")],
        "elixir-ls",
        &[],
        RootRule::Nearest(&["mix.exs"]),
        "brew install elixir-ls, or see https://elixir-lsp.github.io/elixir-ls",
    ),
    def(
        "zig",
        "Zig",
        &[("zig", "zig")],
        "zls",
        &[],
        RootRule::Nearest(&["build.zig", "zls.json"]),
        "brew install zls, or see https://zigtools.org/zls/",
    ),
    def(
        "bash",
        "Bash",
        &[("sh", "shellscript"), ("bash", "shellscript")],
        "bash-language-server",
        &["start"],
        RootRule::Nearest(&[]),
        "npm install -g bash-language-server",
    ),
];

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn language_by_id(id: &str) -> Option<&'static LanguageDef> {
    LANGUAGES.iter().find(|l| l.id == id)
}

/// The language owning a file, by extension.
pub fn language_for(path: &Path) -> Option<&'static LanguageDef> {
    let ext = extension(path);
    if ext.is_empty() {
        return None;
    }
    LANGUAGES
        .iter()
        .find(|l| l.extensions.iter().any(|(e, _)| *e == ext))
}

fn dir_has(dir: &Path, marker: &str) -> bool {
    match marker.strip_prefix("*.") {
        Some(ext) => std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(ext))
            })
        }),
        None => dir.join(marker).exists(),
    }
}

fn declares_workspace(manifest: &Path) -> bool {
    std::fs::read_to_string(manifest)
        .is_ok_and(|text| text.lines().any(|l| l.trim_start().starts_with("[workspace")))
}

/// Folders from the file's directory up to `cwd`, nearest first. `file` must be inside `cwd`.
fn ancestors_within<'a>(file: &'a Path, cwd: &'a Path) -> Vec<&'a Path> {
    file.parent()
        .into_iter()
        .flat_map(|p| p.ancestors())
        .take_while(|dir| dir.starts_with(cwd))
        .collect()
}

/// The root a server for `file` should start in. Never above `cwd`.
pub fn find_root(file: &Path, cwd: &Path, rule: RootRule) -> PathBuf {
    let dirs = ancestors_within(file, cwd);
    let found = match rule {
        RootRule::Nearest(markers) => dirs
            .iter()
            .find(|d| markers.iter().any(|m| dir_has(d, m)))
            .copied(),
        RootRule::NearestPreferring { markers, prefer } => dirs
            .iter()
            .rev()
            .find(|d| prefer.iter().any(|m| dir_has(d, m)))
            .or_else(|| dirs.iter().find(|d| markers.iter().any(|m| dir_has(d, m))))
            .copied(),
        RootRule::CargoWorkspace => dirs
            .iter()
            .find(|d| declares_workspace(&d.join("Cargo.toml")))
            .or_else(|| dirs.iter().find(|d| d.join("Cargo.toml").is_file()))
            .copied(),
    };
    found.unwrap_or(cwd).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        p
    }

    fn rule(id: &str) -> RootRule {
        language_by_id(id).unwrap().root
    }

    #[test]
    fn all_sixteen_languages_and_unique_ids() {
        assert_eq!(LANGUAGES.len(), 16);
        let mut ids: Vec<_> = LANGUAGES.iter().map(|l| l.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn rust_turns_check_on_save_off() {
        let rust = language_by_id("rust").unwrap();
        let opts: serde_json::Value =
            serde_json::from_str(rust.initialization_options.unwrap()).unwrap();
        assert_eq!(opts["checkOnSave"], false);
    }

    #[test]
    fn language_by_extension() {
        assert_eq!(language_for(Path::new("a/b.RS")).unwrap().id, "rust");
        assert_eq!(language_for(Path::new("x.tsx")).unwrap().id, "typescript");
        assert!(language_for(Path::new("notes.txt")).is_none());
        assert!(language_for(Path::new("Makefile")).is_none());
        let ts = language_for(Path::new("x.tsx")).unwrap();
        assert_eq!(ts.lsp_language_id(Path::new("x.tsx")), "typescriptreact");
        assert_eq!(ts.lsp_language_id(Path::new("x.mjs")), "javascript");
    }

    #[test]
    fn vizier_like_layout() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        touch(cwd, "Cargo.toml", "[workspace]\nmembers=[\"crates/*\"]\n");
        touch(cwd, "crates/x/Cargo.toml", "[package]\nname=\"x\"\n");
        let a = touch(cwd, "crates/x/src/lib.rs", "");
        touch(cwd, "app/Cargo.toml", "[workspace]\n");
        touch(cwd, "app/src-tauri/Cargo.toml", "[package]\nname=\"t\"\n");
        let b = touch(cwd, "app/src-tauri/src/main.rs", "");
        touch(cwd, "webui/package.json", "{}");
        let c = touch(cwd, "webui/app/a.ts", "");

        assert_eq!(find_root(&a, cwd, rule("rust")), cwd);
        assert_eq!(find_root(&b, cwd, rule("rust")), cwd.join("app"));
        assert_eq!(find_root(&c, cwd, rule("typescript")), cwd.join("webui"));
    }

    #[test]
    fn gradle_multi_module_prefers_settings() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        touch(cwd, "settings.gradle", "");
        touch(cwd, "app/build.gradle", "");
        let f = touch(cwd, "app/src/Main.java", "");
        assert_eq!(find_root(&f, cwd, rule("java")), cwd);
    }

    #[test]
    fn solution_wins_over_project() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        touch(cwd, "All.sln", "");
        touch(cwd, "src/App/App.csproj", "");
        let f = touch(cwd, "src/App/Program.cs", "");
        assert_eq!(find_root(&f, cwd, rule("csharp")), cwd);
        let lone = tempfile::tempdir().unwrap();
        touch(lone.path(), "src/App/App.csproj", "");
        let g = touch(lone.path(), "src/App/Program.cs", "");
        assert_eq!(find_root(&g, lone.path(), rule("csharp")), lone.path().join("src/App"));
    }

    #[test]
    fn no_marker_means_the_project_folder_and_never_above() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path();
        touch(outer, "package.json", "{}");
        let cwd = outer.join("inner");
        let f = touch(&cwd, "a.ts", "");
        assert_eq!(find_root(&f, &cwd, rule("typescript")), cwd);
    }
}
