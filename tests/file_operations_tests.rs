use oxidized::utils::file::{DirEntry, FileManager};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_filemanager_creation() {
    let file_manager = FileManager::new().unwrap();
    assert_eq!(
        file_manager.current_directory(),
        &std::env::current_dir().unwrap()
    );
}

#[test]
fn test_filemanager_set_directory() {
    let mut file_manager = FileManager::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    assert!(file_manager.change_directory(&temp_path).is_ok());
    assert_eq!(
        file_manager.current_directory(),
        &temp_path.canonicalize().unwrap()
    );
}

#[test]
fn test_filemanager_set_invalid_directory() {
    let mut file_manager = FileManager::new().unwrap();
    let invalid_path = PathBuf::from("/this/path/does/not/exist");

    assert!(file_manager.change_directory(&invalid_path).is_err());
    // Current directory should remain unchanged
    assert_ne!(file_manager.current_directory(), &invalid_path);
}

#[test]
fn test_list_directory_contents() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test files and directories
    fs::write(temp_path.join("test_file.txt"), "test content").unwrap();
    fs::write(temp_path.join("another_file.rs"), "fn main() {}").unwrap();
    fs::create_dir(temp_path.join("test_directory")).unwrap();
    fs::create_dir(temp_path.join("another_dir")).unwrap();

    let mut file_manager = FileManager::new().unwrap();
    file_manager.change_directory(temp_path).unwrap();

    let contents = file_manager.list_directory(temp_path).unwrap();

    // Should have 4 items (2 files + 2 directories)
    assert_eq!(contents.len(), 4);

    // Check that we have the expected items
    let names: Vec<&str> = contents.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"test_file.txt"));
    assert!(names.contains(&"another_file.rs"));
    assert!(names.contains(&"test_directory"));
    assert!(names.contains(&"another_dir"));

    // Check file types using is_dir field
    for entry in &contents {
        match entry.name.as_str() {
            "test_file.txt" | "another_file.rs" => {
                assert!(!entry.is_dir);
            }
            "test_directory" | "another_dir" => {
                assert!(entry.is_dir);
            }
            _ => panic!("Unexpected file: {}", entry.name),
        }
    }
}

#[test]
fn test_list_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let mut file_manager = FileManager::new().unwrap();
    file_manager.change_directory(temp_path).unwrap();

    let contents = file_manager.list_directory(temp_path).unwrap();
    assert!(contents.is_empty());
}

#[test]
fn test_read_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let file_path = temp_path.join("test_read.txt");
    let test_content = "Hello, World!\nThis is a test file.\n";

    fs::write(&file_path, test_content).unwrap();

    let content = FileManager::read_file(&file_path).unwrap();
    assert_eq!(content, test_content);
}

#[test]
fn test_read_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let file_path = temp_path.join("nonexistent.txt");

    let result = FileManager::read_file(&file_path);
    assert!(result.is_err());
}

#[test]
fn test_write_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let file_path = temp_path.join("test_write.txt");
    let test_content = "This is test content for writing.";

    assert!(FileManager::write_file(&file_path, test_content).is_ok());

    // Verify the file was written correctly
    let written_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(written_content, test_content);
}

#[test]
fn test_write_file_to_readonly_directory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a readonly directory (this test might be platform-specific)
    let readonly_dir = temp_path.join("readonly");
    fs::create_dir(&readonly_dir).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444); // readonly
        fs::set_permissions(&readonly_dir, perms).unwrap();

        let file_path = readonly_dir.join("test.txt");
        let result = FileManager::write_file(&file_path, "content");
        assert!(result.is_err());
    }
}

#[test]
fn test_direntry_creation() {
    let entry = DirEntry {
        name: "test.txt".to_string(),
        path: PathBuf::from("/path/to/test.txt"),
        is_dir: false,
        size: 1024,
    };

    assert_eq!(entry.name, "test.txt");
    assert_eq!(entry.path, PathBuf::from("/path/to/test.txt"));
    assert!(!entry.is_dir);
    assert_eq!(entry.size, 1024);
}

#[test]
fn test_directory_listing_sorting() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create files and directories in mixed order
    fs::write(temp_path.join("z_file.txt"), "content").unwrap();
    fs::create_dir(temp_path.join("a_directory")).unwrap();
    fs::write(temp_path.join("b_file.rs"), "fn main() {}").unwrap();
    fs::create_dir(temp_path.join("y_directory")).unwrap();

    let file_manager = FileManager::new().unwrap();
    let contents = file_manager.list_directory(temp_path).unwrap();

    // Should have directories first, then files, both alphabetically sorted
    assert_eq!(contents.len(), 4);

    // Check that directories come first
    assert!(contents[0].is_dir);
    assert!(contents[1].is_dir);
    assert!(!contents[2].is_dir);
    assert!(!contents[3].is_dir);

    // Check alphabetical sorting within each category
    assert_eq!(contents[0].name, "a_directory");
    assert_eq!(contents[1].name, "y_directory");
    assert_eq!(contents[2].name, "b_file.rs");
    assert_eq!(contents[3].name, "z_file.txt");
}

#[test]
fn test_file_extension_handling() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create files with various extensions
    fs::write(temp_path.join("script.rs"), "fn main() {}").unwrap();
    fs::write(temp_path.join("data.json"), "{}").unwrap();
    fs::write(temp_path.join("README"), "readme content").unwrap();
    fs::write(temp_path.join(".hidden"), "hidden file").unwrap();

    let file_manager = FileManager::new().unwrap();
    let contents = file_manager.list_directory(temp_path).unwrap();
    assert_eq!(contents.len(), 4);

    // Verify all files are detected as files
    for entry in &contents {
        assert!(!entry.is_dir);
    }

    // Check that all expected files are present
    let names: Vec<&str> = contents.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"script.rs"));
    assert!(names.contains(&"data.json"));
    assert!(names.contains(&"README"));
    assert!(names.contains(&".hidden"));
}

#[test]
fn test_file_sizes() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let small_content = "small";
    let large_content = "a".repeat(1000);

    fs::write(temp_path.join("small.txt"), small_content).unwrap();
    fs::write(temp_path.join("large.txt"), &large_content).unwrap();

    let file_manager = FileManager::new().unwrap();
    let contents = file_manager.list_directory(temp_path).unwrap();

    for entry in contents {
        match entry.name.as_str() {
            "small.txt" => {
                assert_eq!(entry.size, small_content.len() as u64);
            }
            "large.txt" => {
                assert_eq!(entry.size, large_content.len() as u64);
            }
            _ => panic!("Unexpected file: {}", entry.name),
        }
    }
}

#[test]
fn test_relative_path_navigation() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let sub_dir = temp_path.join("subdir");

    fs::create_dir(&sub_dir).unwrap();
    fs::write(sub_dir.join("file.txt"), "content").unwrap();

    let mut file_manager = FileManager::new().unwrap();
    file_manager.change_directory(temp_path).unwrap();

    // Navigate to subdirectory
    assert!(file_manager.change_directory(&sub_dir).is_ok());

    // Should be able to read file in current directory
    let file_path = sub_dir.join("file.txt");
    let content = FileManager::read_file(&file_path).unwrap();
    assert_eq!(content, "content");
}
