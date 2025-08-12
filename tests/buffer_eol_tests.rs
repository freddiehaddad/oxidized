use oxidized::core::buffer::{Buffer, LineEnding};
use oxidized::core::mode::Position;
use std::fs;
use std::path::PathBuf;

fn temp_file(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    p.push(format!("oxidized_test_{pid}_{name}.txt"));
    p
}

#[test]
fn eol_default_lf_save() {
    let mut b = Buffer::new(1, 50);
    b.lines = vec!["a".into(), "b".into(), "c".into()];
    let path = temp_file("lf_save");
    b.file_path = Some(path.clone());
    // Default eol is LF
    b.save().unwrap();
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, "a\nb\nc");
    let _ = fs::remove_file(path);
}

#[test]
fn eol_save_crlf() {
    let mut b = Buffer::new(2, 50);
    b.lines = vec!["a".into(), "b".into(), "c".into()];
    b.eol = LineEnding::CRLF;
    let path = temp_file("crlf_save");
    b.file_path = Some(path.clone());
    b.save().unwrap();
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, "a\r\nb\r\nc");
    let _ = fs::remove_file(path);
}

#[test]
fn eol_save_cr() {
    let mut b = Buffer::new(3, 50);
    b.lines = vec!["a".into(), "b".into(), "c".into()];
    b.eol = LineEnding::CR;
    let path = temp_file("cr_save");
    b.file_path = Some(path.clone());
    b.save().unwrap();
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, "a\rb\rc");
    let _ = fs::remove_file(path);
}

#[test]
fn from_file_detects_crlf() {
    let path = temp_file("detect_crlf");
    fs::write(&path, "a\r\nb\r\nc").unwrap();
    let b = Buffer::from_file(10, path.clone(), 50).unwrap();
    assert_eq!(b.eol, LineEnding::CRLF);
    assert_eq!(b.lines, vec!["a", "b", "c"]);
    let _ = fs::remove_file(path);
}

#[test]
fn from_file_detects_lf() {
    let path = temp_file("detect_lf");
    fs::write(&path, "a\nb\nc").unwrap();
    let b = Buffer::from_file(11, path.clone(), 50).unwrap();
    assert_eq!(b.eol, LineEnding::LF);
    assert_eq!(b.lines, vec!["a", "b", "c"]);
    let _ = fs::remove_file(path);
}

#[test]
fn from_file_detects_cr() {
    let path = temp_file("detect_cr");
    fs::write(&path, "a\rb\rc").unwrap();
    let b = Buffer::from_file(12, path.clone(), 50).unwrap();
    assert_eq!(b.eol, LineEnding::CR);
    // With CR-only, std::str::lines() does not split; ensure we at least loaded content
    assert_eq!(b.lines.len(), 1);
    assert!(b.lines[0].contains('\r'));
    let _ = fs::remove_file(path);
}

#[test]
fn save_preserves_configured_eol_after_edit() {
    // Prepare CRLF file
    let path = temp_file("preserve_after_edit");
    fs::write(&path, "a\r\nb").unwrap();

    // Load and edit
    let mut b = Buffer::from_file(20, path.clone(), 50).unwrap();
    assert_eq!(b.eol, LineEnding::CRLF);
    b.cursor = Position::new(1, 1);
    b.insert_char('X');

    // Save and verify CRLF between lines
    b.save().unwrap();
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, "a\r\nbX");
    let _ = fs::remove_file(path);
}
