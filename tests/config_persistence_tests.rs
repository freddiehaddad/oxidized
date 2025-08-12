use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

// Utility: run a closure in an isolated temp dir, returning its result.
fn in_temp_dir<F: FnOnce() -> T, T>(f: F) -> T {
    let dir = tempfile::tempdir().expect("tempdir");
    let old = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("chdir temp");
    let result = f();
    std::env::set_current_dir(old).expect("restore cwd");
    result
}

fn read_editor_toml(path: &PathBuf) -> String {
    fs::read_to_string(path).expect("read editor.toml")
}

#[test]
fn set_does_not_persist() {
    let _lock = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    in_temp_dir(|| {
        // Provide a baseline editor.toml copy (minimal) copied from repo root
        let repo_editor = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("editor.toml");
        fs::copy(&repo_editor, "editor.toml").expect("copy baseline editor.toml");
        let path = PathBuf::from("editor.toml");
        let before = read_editor_toml(&path);

        let mut editor = Editor::new().expect("editor construct");

        // Ephemeral changes
        execute_ex_command(&mut editor, "set nu rnu");
        execute_ex_command(&mut editor, "set nocul");
        execute_ex_command(&mut editor, "set tabstop 8");
        assert_eq!(editor.get_config_value("tabstop").as_deref(), Some("8"));

        let after = read_editor_toml(&path);
        assert_eq!(
            before, after,
            ":set unexpectedly persisted changes to editor.toml"
        );
    });
}

#[test]
fn setp_persists() {
    let _lock = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    in_temp_dir(|| {
        let repo_editor = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("editor.toml");
        fs::copy(&repo_editor, "editor.toml").expect("copy baseline editor.toml");
        let path = PathBuf::from("editor.toml");
        let before = read_editor_toml(&path);

        let mut editor = Editor::new().expect("editor construct");
        execute_ex_command(&mut editor, "setp tabstop 6");
        execute_ex_command(&mut editor, "setp number");
        execute_ex_command(&mut editor, "setp cursorline");
        assert_eq!(editor.get_config_value("tabstop").as_deref(), Some("6"));

        let after = read_editor_toml(&path);
        assert_ne!(
            before, after,
            "editor.toml unchanged after :setp operations"
        );
        assert!(
            after.contains("tab_width = 6"),
            ":setp did not persist tabstop change"
        );
    });
}
