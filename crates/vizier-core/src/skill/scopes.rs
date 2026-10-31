//! The skill scopes a turn can see (`specs/041-project-coding-workspace` FR-035–FR-037).
//!
//! Vizier has always had two scopes, both in the workspace: the global `skills/` and one agent's
//! `agents/<id>/skills/`. A *project folder* adds a third, and it is the only one outside the
//! workspace: `.agents/skills` and, as the older home, `.claude/skills` under the turn's `cwd`.
//!
//! Precedence is project → agent → global, and `.agents` before `.claude` inside a project. A slug
//! appears once: the same skill installed in both folders (which this repository does with all 15
//! of its skills) is one line in `/skills`, not two.
//!
//! Project skills are read where they lie, never copied and never indexed: `recommend_skills` runs
//! before a turn's project is resolved, the vector table is one workspace-wide table with no
//! session identity, and the files change under git. [`SkillScopes::discover`] is the read.

use std::path::{Path, PathBuf};

use crate::{
    schema::{AgentId, Skill},
    skill::{SkillManager, SkippedSkill, invocation},
};

/// The project roots a chat looks in, in precedence order.
pub const PROJECT_ROOTS: [&str; 2] = [".agents/skills", ".claude/skills"];

/// Where a skill came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillScope {
    /// The project's own skills. `root` is the folder they were read from, relative to the
    /// project root (`.agents/skills` or `.claude/skills`), so it is also what a person types.
    Project { root: PathBuf },
    Agent(AgentId),
    Global,
}

impl SkillScope {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Project { .. } => "project",
            Self::Agent(_) => "agent",
            Self::Global => "global",
        }
    }

    /// The skill's folder as the person sees it: a project skill's path relative to the project
    /// root (`<root>/<slug>`), and only a name for the workspace scopes.
    pub fn folder(&self, slug: &str) -> String {
        match self {
            Self::Project { root } => root.join(slug).to_string_lossy().into_owned(),
            _ => slug.to_string(),
        }
    }
}

/// A skill and where it was found.
#[derive(Debug, Clone)]
pub struct ScopedSkill {
    pub skill: Skill,
    pub scope: SkillScope,
    /// The skill's own folder on disk: where its resources are, and the one path every read is
    /// confined to.
    pub dir: PathBuf,
}

/// The most files a skill's resource listing returns.
pub const MAX_RESOURCE_FILES: usize = 200;
/// The most bytes of resource paths a listing returns.
pub const MAX_RESOURCE_BYTES: usize = 256 * 1024;
/// Folders never walked for resources: build output and dependency trees are not skill content.
pub const RESOURCE_SKIP_DIRS: [&str; 4] = [".git", "node_modules", ".venv", "target"];

impl ScopedSkill {
    /// The folder this skill's resources live in, relative to the project root for a project
    /// skill. Bodies refer to their own location (`<skill-base-dir>/scripts/…`), so this is what
    /// `use_skill` hands back.
    pub fn folder(&self) -> String {
        self.scope.folder(&self.skill.name)
    }

    /// The resource files of this skill, relative to its folder, bounded and confined
    /// (contract §4): at most [`MAX_RESOURCE_FILES`] paths and [`MAX_RESOURCE_BYTES`] of them,
    /// with binaries and build folders skipped, and `left_out` counting the rest.
    pub fn resources(&self) -> ResourceListing {
        let mut listing = ResourceListing::default();
        let mut bytes = 0usize;
        self.collect(&self.dir, &mut listing, &mut bytes);
        listing.paths.sort();
        listing
    }

    fn collect(&self, dir: &Path, listing: &mut ResourceListing, bytes: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if path.is_dir() {
                if RESOURCE_SKIP_DIRS.contains(&name) {
                    continue;
                }
                self.collect(&path, listing, bytes);
                continue;
            }
            let relative = match path.strip_prefix(&self.dir) {
                Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if relative == "SKILL.md" || relative == ".meta.json" {
                continue;
            }
            if is_binary(&path) {
                continue;
            }
            if listing.paths.len() >= MAX_RESOURCE_FILES || *bytes + relative.len() > MAX_RESOURCE_BYTES {
                listing.left_out += 1;
                continue;
            }
            *bytes += relative.len();
            listing.paths.push(relative);
        }
    }

    /// One resource file of this skill, as text. The path is relative, may not climb out of the
    /// skill's folder, and a binary file is refused by name rather than returned as mojibake.
    pub fn read_resource(&self, path: &str) -> crate::Result<Option<String>> {
        crate::skill::validate_resource_path(path)?;
        let file = self.dir.join(path);
        if !file.is_file() {
            return Ok(None);
        }
        // Canonicalise so a symbolic link out of the skill folder is refused, not followed.
        let real = std::fs::canonicalize(&file)
            .map_err(|err| crate::VizierError(format!("cannot read {path}: {err}")))?;
        let root = std::fs::canonicalize(&self.dir)
            .map_err(|err| crate::VizierError(format!("cannot read {}: {err}", self.dir.display())))?;
        if !real.starts_with(&root) {
            return Err(crate::VizierError(format!(
                "resource_outside_skill: {path} is outside the skill folder"
            )));
        }
        if is_binary(&real) {
            return Err(crate::VizierError(format!(
                "binary_resource: {path} is not text"
            )));
        }
        std::fs::read_to_string(&real)
            .map(Some)
            .map_err(|err| crate::VizierError(format!("cannot read {path}: {err}")))
    }
}

/// The bounded result of [`ScopedSkill::resources`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceListing {
    pub paths: Vec<String>,
    /// Files the caps left out.
    pub left_out: usize,
}

/// A NUL byte in the first 8 KiB is what says "not text" here, the same test the project's walker
/// uses: a heuristic, but it only ever hides a resource from a listing, never corrupts one.
fn is_binary(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    let mut buffer = [0u8; 8192];
    match file.read(&mut buffer) {
        Ok(read) => buffer[..read].contains(&0),
        Err(_) => true,
    }
}

/// One read of every scope.
#[derive(Debug, Clone, Default)]
pub struct Discovered {
    /// Every skill, one entry per slug, highest precedence first.
    pub skills: Vec<ScopedSkill>,
    /// Folders that could not be read. Reported, never silently dropped (FR-039).
    pub skipped: Vec<Skipped>,
}

/// A skill folder that could not be read, named as the person would look for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub folder: String,
    pub reason: String,
}

/// The skill scopes of one turn, highest precedence first.
#[derive(Clone, Default)]
pub struct SkillScopes {
    roots: Vec<(SkillScope, SkillManager)>,
}

impl SkillScopes {
    /// The scopes of a turn: the project's skills when it has a folder, then the agent's own, then
    /// the global ones.
    pub fn for_turn(workspace: &str, agent_id: &AgentId, cwd: Option<&Path>) -> Self {
        let mut roots = Vec::new();
        if let Some(cwd) = cwd {
            for root in PROJECT_ROOTS {
                let dir = cwd.join(root);
                if dir.is_dir() {
                    roots.push((
                        SkillScope::Project { root: PathBuf::from(root) },
                        SkillManager::for_project(&dir),
                    ));
                }
            }
        }
        roots.push((
            SkillScope::Agent(agent_id.clone()),
            SkillManager::for_agent(workspace, agent_id),
        ));
        roots.push((SkillScope::Global, SkillManager::new(workspace)));
        Self { roots }
    }

    /// The workspace scopes only (global and this agent's own) — every non-project turn.
    pub fn workspace(workspace: &str, agent_id: &AgentId) -> Self {
        Self::for_turn(workspace, agent_id, None)
    }

    /// Only the project's own skills, for the system prompt: what this folder carries, in the same
    /// precedence and with the same one-entry-per-slug rule as [`Self::for_turn`].
    pub fn project(cwd: &Path) -> Self {
        let roots = PROJECT_ROOTS
            .iter()
            .map(|root| cwd.join(root))
            .filter(|dir| dir.is_dir())
            .map(|dir| {
                let root = dir
                    .strip_prefix(cwd)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|_| dir.clone());
                (SkillScope::Project { root }, SkillManager::for_project(&dir))
            })
            .collect();
        Self { roots }
    }

    /// Whether this turn sees any project skills at all.
    pub fn has_project(&self) -> bool {
        self.roots.iter().any(|(scope, _)| matches!(scope, SkillScope::Project { .. }))
    }

    /// Every skill of every scope, one entry per slug, highest precedence first, plus what could
    /// not be read.
    pub fn discover(&self) -> Discovered {
        let mut found = Discovered::default();
        for (scope, manager) in &self.roots {
            let scan = match manager.scan() {
                Ok(scan) => scan,
                Err(err) => {
                    // The root itself is unreadable (permissions, a file where a folder belongs):
                    // report it as one skip rather than failing the whole listing.
                    found.skipped.push(Skipped {
                        folder: manager.skills_dir().to_string_lossy().into_owned(),
                        reason: err.to_string(),
                    });
                    continue;
                }
            };
            for skipped in scan.skipped {
                found.skipped.push(Skipped {
                    folder: scope.folder(&skipped.slug),
                    reason: skipped.reason,
                });
            }
            for skill in scan.skills {
                // First scope wins, so a project skill shadows an agent or global one of the same
                // slug and `.agents` shadows `.claude`.
                if found.skills.iter().any(|seen| seen.skill.name == skill.name) {
                    continue;
                }
                let dir = manager.skill_dir(&skill.name);
                found.skills.push(ScopedSkill { skill, scope: scope.clone(), dir });
            }
        }
        found
    }

    /// The skills a person can run, one per slug, sorted by name (`/skills`).
    pub fn available(&self) -> Vec<ScopedSkill> {
        let mut skills = self.discover().skills;
        skills.retain(|scoped| !invocation::is_reserved(&scoped.skill.name));
        skills.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
        skills
    }

    /// The skill `name` resolves to: the highest-precedence scope, an exact case-insensitive match
    /// before one that only matches with `_`/`-` folded.
    pub fn find(&self, name: &str) -> Option<ScopedSkill> {
        if invocation::is_reserved(name) {
            return None;
        }
        let skills = self.discover().skills;
        if let Some(exact) = skills
            .iter()
            .find(|scoped| !invocation::is_reserved(&scoped.skill.name) && scoped.skill.name.eq_ignore_ascii_case(name))
        {
            return Some(exact.clone());
        }
        skills
            .into_iter()
            .find(|scoped| !invocation::is_reserved(&scoped.skill.name) && invocation::same_name(&scoped.skill.name, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, slug: &str, description: &str) {
        let dir = root.join(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {slug}\ndescription: {description}\n---\n\nbody of {slug}\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_slug_in_both_project_roots_appears_once_and_agents_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        write_skill(&cwd.join(".agents/skills"), "standup", "from agents");
        write_skill(&cwd.join(".claude/skills"), "standup", "from claude");
        write_skill(&cwd.join(".claude/skills"), "triage", "only claude");

        let scopes = SkillScopes::for_turn(dir.path().to_str().unwrap(), &"a".to_string(), Some(cwd));
        let names: Vec<String> = scopes.available().into_iter().map(|s| s.skill.name).collect();
        assert_eq!(names, ["standup", "triage"], "one entry per slug");
        assert_eq!(scopes.find("standup").unwrap().skill.description, "from agents");
        assert_eq!(
            scopes.find("standup").unwrap().folder(),
            ".agents/skills/standup",
            "the folder is what a person would type"
        );
    }

    #[test]
    fn a_project_skill_shadows_an_agent_and_a_global_one() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let cwd = dir.path().join("project");
        write_skill(&workspace.join("skills"), "standup", "global");
        write_skill(&workspace.join("agents/a/skills"), "standup", "agent");
        write_skill(&workspace.join("agents/a/skills"), "triage", "agent triage");
        write_skill(&cwd.join(".agents/skills"), "standup", "project");

        let scopes = SkillScopes::for_turn(workspace.to_str().unwrap(), &"a".to_string(), Some(&cwd));
        assert_eq!(scopes.find("standup").unwrap().skill.description, "project");
        assert_eq!(scopes.find("triage").unwrap().scope.label(), "agent");
        assert!(scopes.has_project());
    }

    #[test]
    fn without_a_project_folder_only_the_workspace_scopes_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        write_skill(&workspace.join("skills"), "standup", "global");
        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        assert!(!scopes.has_project());
        assert_eq!(scopes.find("standup").unwrap().scope.label(), "global");
    }

    #[test]
    fn a_folder_without_a_skill_md_is_reported_not_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("skills/broken")).unwrap();
        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        let found = scopes.discover();
        assert!(found.skills.is_empty());
        assert_eq!(found.skipped.len(), 1, "{:?}", found.skipped);
        assert_eq!(found.skipped[0].folder, "broken");
        assert!(found.skipped[0].reason.contains("SKILL.md"), "{:?}", found.skipped[0]);
    }

    #[test]
    fn another_projects_skills_are_not_visible() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let ours = dir.path().join("project-a");
        let theirs = dir.path().join("project-b");
        write_skill(&ours.join(".agents/skills"), "ours", "in a");
        write_skill(&theirs.join(".agents/skills"), "theirs", "in b");

        let scopes = SkillScopes::for_turn(workspace.to_str().unwrap(), &"a".to_string(), Some(&ours));
        assert!(scopes.find("ours").is_some());
        assert!(scopes.find("theirs").is_none(), "another project's skill is not reachable");
    }

    #[test]
    fn a_resource_path_may_not_leave_the_skill_folder() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        write_skill(&workspace.join("skills"), "triage", "t");
        let skill_dir = workspace.join("skills/triage");
        std::fs::write(skill_dir.join("notes.md"), "inside").unwrap();
        std::fs::write(workspace.join("skills/outside.md"), "outside").unwrap();

        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        let scoped = scopes.find("triage").unwrap();

        assert_eq!(scoped.read_resource("notes.md").unwrap().as_deref(), Some("inside"));
        assert!(scoped.read_resource("../outside.md").is_err());
        assert!(scoped.read_resource("/etc/hosts").is_err());
        assert!(scoped.read_resource("missing.md").unwrap().is_none());
    }

    #[test]
    fn a_binary_resource_is_refused_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        write_skill(&workspace.join("skills"), "triage", "t");
        std::fs::write(workspace.join("skills/triage/blob.bin"), [0u8, 1, 2, 3]).unwrap();

        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        let err = scopes.find("triage").unwrap().read_resource("blob.bin").unwrap_err();
        assert!(err.to_string().contains("binary_resource"), "{err}");
        assert!(
            !scopes.find("triage").unwrap().resources().paths.contains(&"blob.bin".to_string()),
            "and it is not listed either"
        );
    }

    #[test]
    fn the_resource_listing_is_capped_and_says_how_many_were_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        write_skill(&workspace.join("skills"), "many", "m");
        let skill_dir = workspace.join("skills/many");
        std::fs::create_dir_all(skill_dir.join("node_modules")).unwrap();
        std::fs::write(skill_dir.join("node_modules/skip.md"), "not a resource").unwrap();
        for n in 0..(MAX_RESOURCE_FILES + 5) {
            std::fs::write(skill_dir.join(format!("file-{n:04}.md")), "x").unwrap();
        }

        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        let listing = scopes.find("many").unwrap().resources();
        assert_eq!(listing.paths.len(), MAX_RESOURCE_FILES);
        assert_eq!(listing.left_out, 5);
        assert!(!listing.paths.iter().any(|path| path.starts_with("node_modules")));
        assert!(!listing.paths.contains(&"SKILL.md".to_string()), "SKILL.md is the skill, not a resource");
    }

    #[test]
    fn reserved_names_never_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        write_skill(&workspace.join("skills"), "skills", "on disk, never invoked");
        let scopes = SkillScopes::workspace(workspace.to_str().unwrap(), &"a".to_string());
        assert!(scopes.find("skills").is_none());
        assert!(scopes.available().is_empty());
    }
}
