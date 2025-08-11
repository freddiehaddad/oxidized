use oxidized::utils::command::Command;

#[test]
fn test_command_parse_simple() {
    let command = Command::parse("quit").unwrap();
    assert_eq!(command.name, "quit");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_with_args() {
    let command = Command::parse("edit filename.txt").unwrap();
    assert_eq!(command.name, "edit");
    assert_eq!(command.args, vec!["filename.txt"]);
}

#[test]
fn test_command_parse_multiple_args() {
    let command = Command::parse("substitute old new g").unwrap();
    assert_eq!(command.name, "substitute");
    assert_eq!(command.args, vec!["old", "new", "g"]);
}

#[test]
fn test_command_parse_with_whitespace() {
    let command = Command::parse("  write  file.txt  ").unwrap();
    assert_eq!(command.name, "write");
    assert_eq!(command.args, vec!["file.txt"]);
}

#[test]
fn test_command_parse_empty_string() {
    let result = Command::parse("");
    assert!(result.is_none());
}

#[test]
fn test_command_parse_whitespace_only() {
    let result = Command::parse("   ");
    assert!(result.is_none());
}

#[test]
fn test_command_parse_single_letter_commands() {
    let command = Command::parse("q").unwrap();
    assert_eq!(command.name, "q");
    assert!(command.args.is_empty());

    let command = Command::parse("w").unwrap();
    assert_eq!(command.name, "w");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_numbers() {
    let command = Command::parse("123").unwrap();
    assert_eq!(command.name, "123");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_with_colon() {
    // Should handle commands without leading colon
    let command = Command::parse("quit!").unwrap();
    assert_eq!(command.name, "quit!");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_ex_commands() {
    // Test common Ex commands
    let commands: Vec<(&str, &str, Vec<&str>)> = vec![
        ("q", "q", vec![]),
        ("quit", "quit", vec![]),
        ("q!", "q!", vec![]),
        ("w", "w", vec![]),
        ("write", "write", vec![]),
        ("wq", "wq", vec![]),
        ("x", "x", vec![]),
        ("split", "split", vec![]),
        ("vsplit", "vsplit", vec![]),
        ("sp", "sp", vec![]),
        ("vsp", "vsp", vec![]),
        ("close", "close", vec![]),
        ("help", "help", vec![]),
        ("set", "set", vec![]),
        ("buffers", "buffers", vec![]),
        ("ls", "ls", vec![]),
    ];

    for (input, expected_name, expected_args) in commands {
        let command = Command::parse(input).unwrap();
        assert_eq!(command.name, expected_name);
        assert_eq!(command.args, expected_args);
    }
}

#[test]
fn test_command_parse_file_operations() {
    // Test file operation commands with arguments
    let test_cases: Vec<(&str, &str, Vec<&str>)> = vec![
        ("e file.txt", "e", vec!["file.txt"]),
        ("edit /path/to/file.rs", "edit", vec!["/path/to/file.rs"]),
        ("w file.txt", "w", vec!["file.txt"]),
        (
            "write /home/user/document.md",
            "write",
            vec!["/home/user/document.md"],
        ),
        ("r input.txt", "r", vec!["input.txt"]),
        ("read data.txt", "read", vec!["data.txt"]),
    ];

    for (input, expected_name, expected_args) in test_cases {
        let command = Command::parse(input).unwrap();
        assert_eq!(command.name, expected_name);
        assert_eq!(command.args, expected_args);
    }
}

#[test]
fn test_command_parse_set_commands() {
    // Test :set commands with various options
    let test_cases: Vec<(&str, &str, Vec<&str>)> = vec![
        ("set number", "set", vec!["number"]),
        ("set nonumber", "set", vec!["nonumber"]),
        ("set tabstop=4", "set", vec!["tabstop=4"]),
        ("set shiftwidth=2", "set", vec!["shiftwidth=2"]),
        ("set expandtab", "set", vec!["expandtab"]),
        ("set noexpandtab", "set", vec!["noexpandtab"]),
        ("set hlsearch", "set", vec!["hlsearch"]),
        ("set nohlsearch", "set", vec!["nohlsearch"]),
        ("set ignorecase", "set", vec!["ignorecase"]),
        ("set smartcase", "set", vec!["smartcase"]),
    ];

    for (input, expected_name, expected_args) in test_cases {
        let command = Command::parse(input).unwrap();
        assert_eq!(command.name, expected_name);
        assert_eq!(command.args, expected_args);
    }
}

#[test]
fn test_command_parse_search_replace() {
    let command = Command::parse("s/old/new/").unwrap();
    assert_eq!(command.name, "s/old/new/");
    assert!(command.args.is_empty());

    let command = Command::parse("substitute/pattern/replacement/g").unwrap();
    assert_eq!(command.name, "substitute/pattern/replacement/g");
    assert!(command.args.is_empty());

    let command = Command::parse("%s/foo/bar/gi").unwrap();
    assert_eq!(command.name, "%s/foo/bar/gi");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_line_numbers() {
    let command = Command::parse("42").unwrap();
    assert_eq!(command.name, "42");
    assert!(command.args.is_empty());

    let command = Command::parse("1,10d").unwrap();
    assert_eq!(command.name, "1,10d");
    assert!(command.args.is_empty());

    let command = Command::parse("1,$s/old/new/g").unwrap();
    assert_eq!(command.name, "1,$s/old/new/g");
    assert!(command.args.is_empty());

    let command = Command::parse(".,.+5d").unwrap();
    assert_eq!(command.name, ".,.+5d");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_buffer_commands() {
    let command = Command::parse("b 1").unwrap();
    assert_eq!(command.name, "b");
    assert_eq!(command.args, vec!["1"]);

    let command = Command::parse("buffer main.rs").unwrap();
    assert_eq!(command.name, "buffer");
    assert_eq!(command.args, vec!["main.rs"]);

    let command = Command::parse("bd").unwrap();
    assert_eq!(command.name, "bd");
    assert!(command.args.is_empty());

    let command = Command::parse("bdelete").unwrap();
    assert_eq!(command.name, "bdelete");
    assert!(command.args.is_empty());

    let command = Command::parse("bn").unwrap();
    assert_eq!(command.name, "bn");
    assert!(command.args.is_empty());

    let command = Command::parse("bnext").unwrap();
    assert_eq!(command.name, "bnext");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_window_commands() {
    let command = Command::parse("split").unwrap();
    assert_eq!(command.name, "split");
    assert!(command.args.is_empty());

    let command = Command::parse("sp").unwrap();
    assert_eq!(command.name, "sp");
    assert!(command.args.is_empty());

    let command = Command::parse("vsplit").unwrap();
    assert_eq!(command.name, "vsplit");
    assert!(command.args.is_empty());

    let command = Command::parse("split file.txt").unwrap();
    assert_eq!(command.name, "split");
    assert_eq!(command.args, vec!["file.txt"]);

    let command = Command::parse("vsplit /path/to/file").unwrap();
    assert_eq!(command.name, "vsplit");
    assert_eq!(command.args, vec!["/path/to/file"]);

    let command = Command::parse("wincmd h").unwrap();
    assert_eq!(command.name, "wincmd");
    assert_eq!(command.args, vec!["h"]);

    let command = Command::parse("resize +5").unwrap();
    assert_eq!(command.name, "resize");
    assert_eq!(command.args, vec!["+5"]);

    let command = Command::parse("vertical resize 80").unwrap();
    assert_eq!(command.name, "vertical");
    assert_eq!(command.args, vec!["resize", "80"]);
}

#[test]
fn test_command_parse_help_commands() {
    let command = Command::parse("help").unwrap();
    assert_eq!(command.name, "help");
    assert!(command.args.is_empty());

    let command = Command::parse("h").unwrap();
    assert_eq!(command.name, "h");
    assert!(command.args.is_empty());

    let command = Command::parse("help commands").unwrap();
    assert_eq!(command.name, "help");
    assert_eq!(command.args, vec!["commands"]);

    let command = Command::parse("help :w").unwrap();
    assert_eq!(command.name, "help");
    assert_eq!(command.args, vec![":w"]);
}

#[test]
fn test_command_parse_shell_commands() {
    let command = Command::parse("!ls").unwrap();
    assert_eq!(command.name, "!ls");
    assert!(command.args.is_empty());

    let command = Command::parse("!pwd").unwrap();
    assert_eq!(command.name, "!pwd");
    assert!(command.args.is_empty());

    let command = Command::parse("!make clean").unwrap();
    assert_eq!(command.name, "!make");
    assert_eq!(command.args, vec!["clean"]);

    let command = Command::parse("!cargo build").unwrap();
    assert_eq!(command.name, "!cargo");
    assert_eq!(command.args, vec!["build"]);

    let command = Command::parse("shell").unwrap();
    assert_eq!(command.name, "shell");
    assert!(command.args.is_empty());
}

#[test]
fn test_command_parse_complex_filenames() {
    let command = Command::parse("e file with spaces.txt").unwrap();
    assert_eq!(command.name, "e");
    assert_eq!(command.args, vec!["file", "with", "spaces.txt"]);

    let command = Command::parse("write my-file_name.rs").unwrap();
    assert_eq!(command.name, "write");
    assert_eq!(command.args, vec!["my-file_name.rs"]);

    let command = Command::parse("edit file.with.dots.ext").unwrap();
    assert_eq!(command.name, "edit");
    assert_eq!(command.args, vec!["file.with.dots.ext"]);
}

#[test]
fn test_command_parse_numeric_arguments() {
    let command = Command::parse("set tabstop=8").unwrap();
    assert_eq!(command.name, "set");
    assert_eq!(command.args, vec!["tabstop=8"]);

    let command = Command::parse("resize 50").unwrap();
    assert_eq!(command.name, "resize");
    assert_eq!(command.args, vec!["50"]);

    let command = Command::parse("buffer 3").unwrap();
    assert_eq!(command.name, "buffer");
    assert_eq!(command.args, vec!["3"]);
}

#[test]
fn test_command_parse_special_characters() {
    let command = Command::parse("q!").unwrap();
    assert_eq!(command.name, "q!");
    assert!(command.args.is_empty());

    let command = Command::parse("w!").unwrap();
    assert_eq!(command.name, "w!");
    assert!(command.args.is_empty());

    let command = Command::parse("wq!").unwrap();
    assert_eq!(command.name, "wq!");
    assert!(command.args.is_empty());

    let command = Command::parse("edit!").unwrap();
    assert_eq!(command.name, "edit!");
    assert!(command.args.is_empty());
}
