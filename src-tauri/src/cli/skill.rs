//! `install-skill` (v0.5.0 phase 35): writes the Claude Code Skill
//! `higiri` to the personal skills folder. The Skill is one `SKILL.md`,
//! built into the CLI so that the two do not drift apart.
//!
//! The folder is `skills/higiri/` in Claude Code's configuration folder:
//! `CLAUDE_CONFIG_DIR` when it is set, otherwise `.claude` in the home
//! folder. The environment is read only by [`install_for_user`]; the rest
//! takes its folders as arguments, for the tests. No database is opened.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::error::CliError;

/// The Skill (`src/cli/SKILL.md`).
pub const SKILL_MD: &str = include_str!("SKILL.md");

/// The Skill's name: its folder under `skills/`.
pub const SKILL_NAME: &str = "higiri";

const FILE_NAME: &str = "SKILL.md";

/// What `install-skill` writes to standard output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledSkill {
    /// The `SKILL.md` written.
    pub path: String,
}

/// Claude Code's configuration folder: `config_dir` (`CLAUDE_CONFIG_DIR`)
/// unless it is unset or empty, otherwise `.claude` in `home`. `None` when
/// neither is known.
pub fn config_folder(config_dir: Option<OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    match config_dir {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => home.map(|home| home.join(".claude")),
    }
}

/// The Skill's folder in the configuration folder `config`.
pub fn skill_folder(config: &Path) -> PathBuf {
    config.join("skills").join(SKILL_NAME)
}

/// Writes [`SKILL_MD`] as `SKILL.md` in `folder`, which is made when it is
/// missing. A `SKILL.md` there is replaced; nothing else in the folder is
/// touched. The file is written as `SKILL.md.part` first and renamed, so a
/// failure leaves the old one (or none), never half of it.
pub fn install(folder: &Path) -> Result<InstalledSkill, CliError> {
    let path = folder.join(FILE_NAME);
    let part = folder.join(format!("{FILE_NAME}.part"));
    let written = std::fs::create_dir_all(folder)
        .and_then(|()| {
            let mut file = std::fs::File::create(&part)?;
            file.write_all(SKILL_MD.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&part, &path));
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
        return Err(CliError::skill_not_written(&path));
    }
    Ok(InstalledSkill {
        path: path.display().to_string(),
    })
}

/// `install-skill`: [`install`] in the folder of this user's Claude Code
/// (the environment's `CLAUDE_CONFIG_DIR`, or the home folder). A relative
/// `CLAUDE_CONFIG_DIR` is taken from the current folder.
pub fn install_for_user() -> Result<InstalledSkill, CliError> {
    let config = config_folder(std::env::var_os("CLAUDE_CONFIG_DIR"), dirs::home_dir())
        .ok_or_else(CliError::no_home_folder)?;
    let config = std::path::absolute(&config).unwrap_or(config);
    install(&skill_folder(&config))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use clap::CommandFactory;

    use super::*;
    use crate::cli::args::Cli;
    use crate::cli::error::CliErrorKind;

    /// The sub-commands that only read: the Skill runs them without asking.
    const READS: [&str; 5] = ["today", "list", "show", "search", "projects"];

    /// The front matter's keys and values. A value is a string, or the
    /// items of a list written as `  - item` lines under the key.
    fn front_matter(skill: &str) -> Vec<(String, Vec<String>)> {
        let mut lines = skill.lines();
        assert_eq!(lines.next(), Some("---"), "the front matter comes first");
        let mut fields: Vec<(String, Vec<String>)> = Vec::new();
        for line in lines {
            if line == "---" {
                return fields;
            }
            if let Some(item) = line.strip_prefix("  - ") {
                let (_, items) = fields.last_mut().expect("a list under a key");
                items.push(item.to_owned());
                continue;
            }
            let (key, value) = line.split_once(':').expect("key: value");
            let value = value.trim();
            let values = if value.is_empty() {
                Vec::new()
            } else {
                vec![value.to_owned()]
            };
            fields.push((key.to_owned(), values));
        }
        panic!("the front matter is not closed");
    }

    fn field<'a>(fields: &'a [(String, Vec<String>)], key: &str) -> &'a [String] {
        &fields
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("{key} is missing"))
            .1
    }

    /// The CLI's sub-commands, as typed.
    fn sub_commands() -> Vec<String> {
        Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .collect()
    }

    #[test]
    fn the_front_matter_names_the_skill() {
        let fields = front_matter(SKILL_MD);
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["name", "description", "allowed-tools"]);
        assert_eq!(field(&fields, "name"), [SKILL_NAME]);
        assert_eq!(
            field(&fields, "description"),
            [
                "Higiri(TODO管理アプリ)のTODOを読み、追加し、ステータスを変え、作業ログを付ける。今日やるべき作業を提案する。TODO、日限、やること、作業ログ、日報の材料の話が出たら使う。"
            ]
        );
    }

    #[test]
    fn only_the_reads_run_without_asking() {
        let fields = front_matter(SKILL_MD);
        let allowed: BTreeSet<&str> = field(&fields, "allowed-tools")
            .iter()
            .map(|tool| {
                tool.strip_prefix("Bash(higiri-cli ")
                    .and_then(|rest| rest.strip_suffix(" *)"))
                    .unwrap_or_else(|| panic!("{tool}: Bash(higiri-cli <sub-command> *)"))
            })
            .collect();
        assert_eq!(allowed, BTreeSet::from(READS));
        assert_eq!(field(&fields, "allowed-tools").len(), READS.len());

        // Every other sub-command asks first, and the reads are still
        // sub-commands of the CLI.
        let subs = sub_commands();
        for read in READS {
            assert!(subs.iter().any(|s| s == read), "{read}");
        }
        let writes: Vec<&String> = subs
            .iter()
            .filter(|s| !READS.contains(&s.as_str()))
            .collect();
        assert_eq!(
            writes,
            [
                "add",
                "update",
                "status",
                "log",
                "append-memo",
                "install-skill"
            ]
        );
        for write in writes {
            assert!(!allowed.contains(write.as_str()), "{write}");
        }
        // No pattern that would take every sub-command.
        assert!(!SKILL_MD.contains("Bash(higiri-cli *)"));
        assert!(!SKILL_MD.contains("Bash(higiri-cli:*)"));
    }

    #[test]
    fn the_body_tells_every_sub_command_but_install_skill() {
        for sub in sub_commands() {
            let told = SKILL_MD.contains(&format!("`higiri-cli {sub}"));
            assert_eq!(told, sub != "install-skill", "{sub}");
        }
        // The flags a refusal asks for.
        for flag in ["--despite-blocked", "--with-children", "--body -"] {
            assert!(SKILL_MD.contains(flag), "{flag}");
        }
    }

    #[test]
    fn the_folder_is_claude_config_dir_or_dot_claude() {
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(
            config_folder(None, home.clone()),
            Some(PathBuf::from("/home/u/.claude"))
        );
        assert_eq!(
            config_folder(Some("/work/claude".into()), home.clone()),
            Some(PathBuf::from("/work/claude"))
        );
        // Set but empty: as if unset.
        assert_eq!(
            config_folder(Some(OsString::new()), home),
            Some(PathBuf::from("/home/u/.claude"))
        );
        assert_eq!(
            config_folder(Some("/work/claude".into()), None),
            Some(PathBuf::from("/work/claude"))
        );
        assert_eq!(config_folder(None, None), None);
        assert_eq!(
            skill_folder(Path::new("/work/claude")),
            Path::new("/work/claude/skills/higiri")
        );
    }

    #[test]
    fn install_makes_the_folders_and_replaces_the_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = skill_folder(&tmp.path().join("claude"));
        let path = folder.join("SKILL.md");

        let installed = install(&folder).unwrap();
        assert_eq!(installed.path, path.display().to_string());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SKILL_MD);
        assert_eq!(
            serde_json::to_string(&installed).unwrap(),
            serde_json::json!({ "path": path.display().to_string() }).to_string()
        );

        // Again, over an older Skill and beside a file of the user's.
        std::fs::write(&path, "古い Skill").unwrap();
        std::fs::write(folder.join("notes.md"), "利用者のメモ").unwrap();
        assert_eq!(install(&folder).unwrap(), installed);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SKILL_MD);
        assert_eq!(
            std::fs::read_to_string(folder.join("notes.md")).unwrap(),
            "利用者のメモ"
        );
        let mut names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["SKILL.md", "notes.md"]);
    }

    #[test]
    fn a_folder_it_cannot_write_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        // A file where the folder would be.
        let blocker = tmp.path().join("claude");
        std::fs::write(&blocker, "").unwrap();
        let folder = skill_folder(&blocker);
        let e = install(&folder).unwrap_err();
        assert_eq!(e.kind, CliErrorKind::Internal);
        assert_eq!(
            e.message,
            format!(
                "Skill を書けませんでした({})。書き込めるフォルダか確かめてから、もう一度実行してください。",
                folder.join("SKILL.md").display()
            )
        );
        assert!(!folder.join("SKILL.md.part").exists());
    }
}
