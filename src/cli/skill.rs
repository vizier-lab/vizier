use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::VizierConfig;
use crate::skill::{SkillManager, install};

#[derive(Debug, Args, Clone)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SkillCommand {
    /// Install a skill from registry, git repo, or local path
    Install {
        /// Skill slug (for registry) or git URL or local path
        source: String,
        /// Install for a specific agent (optional)
        #[arg(short, long)]
        agent: Option<String>,
    },
    /// List installed skills
    List {
        /// Filter by activation mode (always, on_demand, contextual)
        #[arg(short, long)]
        activation: Option<String>,
    },
    /// Uninstall a skill
    Uninstall {
        /// Skill slug to uninstall
        slug: String,
        /// Uninstall from a specific agent (optional)
        #[arg(short, long)]
        agent: Option<String>,
    },
    /// Update a skill from its source
    Update {
        /// Skill slug to update
        slug: String,
    },
}

use clap::Args;

pub fn skill(args: SkillArgs) -> Result<()> {
    let config = VizierConfig::load(None)?;
    let workspace = &config.workspace;

    match args.command {
        SkillCommand::Install { source, agent } => {
            let target_dir = if let Some(ref agent_id) = agent {
                std::path::PathBuf::from(workspace)
                    .join("agents")
                    .join(agent_id)
                    .join("skills")
            } else {
                std::path::PathBuf::from(workspace).join("skills")
            };

            let manager = if let Some(ref agent_id) = agent {
                SkillManager::for_agent(workspace, agent_id)
            } else {
                SkillManager::new(workspace)
            };

            println!("Installing skill from '{}'...", source);

            match install::install_skill(&manager, &source, &target_dir) {
                Ok(installed) => {
                    for slug in &installed {
                        println!("✓ Installed skill: {}", slug);
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to install skill: {}", e);
                    std::process::exit(1);
                }
            }
        }
        SkillCommand::List { activation } => {
            let manager = SkillManager::new(workspace);
            let skills = manager.list_skills()?;

            let filtered: Vec<_> = if let Some(ref act) = activation {
                skills
                    .iter()
                    .filter(|s| format!("{:?}", s.activation).to_lowercase() == act.to_lowercase())
                    .collect()
            } else {
                skills.iter().collect()
            };

            if filtered.is_empty() {
                println!("No skills installed.");
                return Ok(());
            }

            println!("{:<20} {:<30} {:<15} {:<10}", "NAME", "DESCRIPTION", "ACTIVATION", "VERSION");
            println!("{}", "-".repeat(75));

            for skill in filtered {
                let desc = if skill.description.len() > 28 {
                    format!("{}...", &skill.description[..25])
                } else {
                    skill.description.clone()
                };
                println!(
                    "{:<20} {:<30} {:<15} {:<10}",
                    skill.name,
                    desc,
                    format!("{:?}", skill.activation),
                    skill.version
                );
            }
        }
        SkillCommand::Uninstall { slug, agent } => {
            let manager = if let Some(ref agent_id) = agent {
                SkillManager::for_agent(workspace, agent_id)
            } else {
                SkillManager::new(workspace)
            };

            match manager.delete_skill(&slug) {
                Ok(true) => println!("✓ Uninstalled skill: {}", slug),
                Ok(false) => {
                    eprintln!("✗ Skill '{}' not found", slug);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("✗ Failed to uninstall skill: {}", e);
                    std::process::exit(1);
                }
            }
        }
        SkillCommand::Update { slug } => {
            let manager = SkillManager::new(workspace);
            let meta = manager.load_meta(&slug)?;

            let meta = match meta {
                Some(m) => m,
                None => {
                    eprintln!("✗ Skill '{}' was not installed via CLI (no .meta.json)", slug);
                    std::process::exit(1);
                }
            };

            if meta.source != crate::skill::SkillSource::Registry {
                eprintln!("✗ Skill '{}' was not installed from registry, cannot update", slug);
                std::process::exit(1);
            }

            let registry_url = meta.registry_url.as_deref().unwrap();
            let target_dir = std::path::PathBuf::from(workspace).join("skills");

            println!("Updating skill '{}' from registry...", slug);

            match install::install_skill(&manager, registry_url, &target_dir) {
                Ok(_) => {
                    println!("✓ Updated skill: {}", slug);
                }
                Err(e) => {
                    eprintln!("✗ Failed to update skill: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
