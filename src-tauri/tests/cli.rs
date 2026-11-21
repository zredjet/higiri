//! The higiri-cli executable itself: where it writes and its exit codes.
//!
//! Only `--help`, `--version` and wrong arguments run everywhere (they read
//! no data; on Windows, no test may read or write the user's data). The runs that open a database need a home folder of their own,
//! so they are on macOS only (`HOME` decides the app data folder there): on
//! Windows the folder is the user's, whatever the environment says.

use std::process::Command;

fn cli(args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_higiri-cli"));
    c.args(args);
    c
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("UTF-8")
}

#[test]
fn help_and_version_go_to_stdout_with_0() {
    let out = cli(&["--help"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("今日のまとめ"));
    assert!(out.stderr.is_empty());

    let out = cli(&["search", "--help"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("探す語"));

    let out = cli(&["--version"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        text(&out.stdout),
        format!("higiri-cli {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn wrong_arguments_go_to_stderr_with_2() {
    for args in [
        &["--dev"][..],
        &["list", "--status", "finished"],
        &["show", "x"],
    ] {
        let out = cli(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert!(!out.stderr.is_empty(), "{args:?}");
    }
}

/// `install-skill` writes under `CLAUDE_CONFIG_DIR`, which the test sets for
/// the child only, so it runs everywhere without touching the user's
/// Claude Code. The home folder has no data: no database is opened.
#[test]
fn install_skill_writes_under_claude_config_dir_and_replaces() {
    let config = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let skill = config.path().join("skills").join("higiri").join("SKILL.md");
    let install = || {
        cli(&["install-skill"])
            .env("CLAUDE_CONFIG_DIR", config.path())
            .env("HOME", home.path())
            .output()
            .unwrap()
    };

    let out = install();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(out.stderr.is_empty());
    let expected = format!(
        "{}\n",
        serde_json::json!({ "path": skill.display().to_string() })
    );
    assert_eq!(text(&out.stdout), expected);
    let written = std::fs::read_to_string(&skill).unwrap();
    assert_eq!(written, include_str!("../src/cli/SKILL.md"));

    std::fs::write(&skill, "古い Skill").unwrap();
    let out = install();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(text(&out.stdout), expected);
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), written);

    // Nothing was made for the data.
    assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
}

#[cfg(target_os = "macos")]
mod with_a_home_of_its_own {
    use std::process::Output;

    use super::*;

    fn run(home: &std::path::Path, args: &[&str]) -> Output {
        cli(args).env("HOME", home).output().unwrap()
    }

    #[test]
    fn no_data_is_an_error_on_stderr_with_1_and_makes_nothing() {
        let home = tempfile::tempdir().unwrap();
        for args in [&["today"][..], &["--dev", "projects"]] {
            let out = run(home.path(), args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
            assert!(out.stdout.is_empty());
            assert_eq!(
                text(&out.stderr),
                "{\"error\":{\"kind\":\"not_found\",\"message\":\"Higiri のデータが見つかりません。先に Higiri を一度起動してください。\"}}\n"
            );
        }
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 0);
    }

    #[test]
    fn data_of_the_development_build_with_dev() {
        let home = tempfile::tempdir().unwrap();
        let dev = home
            .path()
            .join("Library/Application Support/com.redjet.higiri/dev");
        std::fs::create_dir_all(&dev).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            higiri_lib::repo::db::open(&dev.join("todo.db"))
                .await
                .unwrap()
                .close()
                .await;
        });

        let out = run(home.path(), &["projects", "--dev"]);
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(
            text(&out.stdout),
            "{\"projects\":[],\"hidden_projects\":0}\n"
        );
        assert!(out.stderr.is_empty());
        // Without --dev: the release build's data, which is not there.
        let out = run(home.path(), &["projects"]);
        assert_eq!(out.status.code(), Some(1));
    }

    #[test]
    fn writes_read_a_body_from_stdin_and_take_the_days_backup() {
        use std::io::Write;
        use std::process::Stdio;

        let home = tempfile::tempdir().unwrap();
        let dev = home
            .path()
            .join("Library/Application Support/com.redjet.higiri/dev");
        std::fs::create_dir_all(&dev).unwrap();
        let today = higiri_lib::commands::clock::today();
        let now = higiri_lib::commands::clock::now();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // A TODO waiting for another one (the sample data is not in release
        // builds of the library).
        let later = runtime.block_on(async {
            use higiri_lib::commands::{dependencies, projects, todos};
            let pool = higiri_lib::repo::db::open(&dev.join("todo.db"))
                .await
                .unwrap();
            let project = projects::create(
                &pool,
                projects::ProjectInput {
                    name: "試す".into(),
                    color: None,
                    description: String::new(),
                },
                now,
            )
            .await
            .unwrap();
            let mut made = Vec::new();
            for title in ["先に終わらせる", "あとで始める"] {
                let input = todos::NewTodoInput {
                    project_id: project.id,
                    parent_id: None,
                    template_id: None,
                    title: title.into(),
                    priority: None,
                    due_date: None,
                    planned_start: None,
                    planned_end: None,
                    memo_md: None,
                };
                made.push(
                    todos::create(&pool, input, today, now)
                        .await
                        .unwrap()
                        .todo
                        .id,
                );
            }
            dependencies::add(&pool, made[1], made[0], today, now)
                .await
                .unwrap();
            pool.close().await;
            made[1]
        });
        let id = later.to_string();

        let mut child = cli(&["--dev", "log", &id, "--body", "-"])
            .env("HOME", home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all("## 今日\n\n- 範囲の案を作った\n".as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert!(out.stderr.is_empty());
        let log: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(log["body_md"], "## 今日\n\n- 範囲の案を作った");
        assert_eq!(log["log_date"], today.to_string());
        assert!(dev.join(format!("backups/todo-{today}.db")).is_file());

        // What the screen asks first: on standard error, with 1.
        let out = run(home.path(), &["--dev", "status", &id, "doing"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(
            text(&out.stderr).starts_with("{\"error\":{\"kind\":\"needs_confirmation\""),
            "{}",
            text(&out.stderr)
        );
    }
}
