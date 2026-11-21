//! The higiri-cli executable itself: where it writes and its exit codes.
//!
//! Only `--help`, `--version` and wrong arguments run everywhere (they read
//! no data). The runs that open a database need a home folder of their own,
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
}
