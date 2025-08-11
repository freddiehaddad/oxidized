// Command parsing and execution
// This handles Ex-style commands (:w, :q, etc.) in a central place.

use log::{debug, warn};

use crate::core::editor::Editor;
use crate::core::mode::Mode;

pub struct Command {
    pub name: String,
    pub args: Vec<String>,
}

impl Command {
    pub fn parse(input: &str) -> Option<Self> {
        debug!("Parsing command: '{}'", input);
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            warn!("Empty command input received");
            return None;
        }

        let name = parts[0].to_string();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        debug!(
            "Parsed command '{}' with {} args: {:?}",
            name,
            args.len(),
            args
        );

        Some(Self { name, args })
    }
}

/// Execute an Ex-style command against the editor
///
/// Inputs
/// - editor: mutable reference to the editor instance
/// - raw: command string without leading ':' (e.g., "w", "bd!", "e file.txt")
///
/// Behavior
/// - Mutates editor state, updates status message, and may request quit
/// - Always returns with editor back in Normal mode and command line cleared
pub fn execute_ex_command(editor: &mut Editor, raw: &str) {
    let command = raw.trim();

    match command {
        // Quit commands
        "q" | "quit" => editor.quit(),
        "q!" | "quit!" => editor.force_quit(),

        // Write/save commands
        "w" | "write" => {
            if let Err(e) = editor.save_current_buffer() {
                editor.set_status_message(format!("Error saving: {}", e));
            }
        }

        // Write and quit
        "wq" | "x" => {
            if let Err(e) = editor.save_current_buffer() {
                editor.set_status_message(format!("Error saving: {}", e));
            } else {
                editor.quit();
            }
        }
        // Force write and quit
        "wq!" | "x!" => {
            if let Err(e) = editor.save_current_buffer_force() {
                editor.set_status_message(format!("Error saving: {}", e));
            } else {
                editor.quit();
            }
        }

        // Create a new empty buffer and switch to it
        "enew" => match editor.create_buffer(None) {
            Ok(id) => editor.set_status_message(format!("New empty buffer {}", id)),
            Err(e) => editor.set_status_message(format!("Error creating buffer: {}", e)),
        },

        // Line number toggles
        "set nu" | "set number" => editor.set_line_numbers(true, false),
        "set nonu" | "set nonumber" => editor.set_line_numbers(false, false),
        "set rnu" | "set relativenumber" => editor.set_line_numbers(false, true),
        "set nornu" | "set norelativenumber" => editor.set_line_numbers(true, false),
        "set nu rnu" | "set number relativenumber" => editor.set_line_numbers(true, true),

        // Cursor line toggles
        "set cul" | "set cursorline" => editor.set_cursor_line(true),
        "set nocul" | "set nocursorline" => editor.set_cursor_line(false),

        // Buffer management
        "bn" | "bnext" => {
            if editor.switch_to_next_buffer() {
                editor.set_status_message("Switched to next buffer".to_string());
            } else {
                editor.set_status_message("No next buffer".to_string());
            }
        }
        "bd" | "bdelete" => match editor.close_current_buffer() {
            Ok(msg) => editor.set_status_message(msg),
            Err(e) => editor.set_status_message(format!("Error: {}", e)),
        },
        "bd!" | "bdelete!" => match editor.force_close_current_buffer() {
            Ok(msg) => editor.set_status_message(msg),
            Err(e) => editor.set_status_message(format!("Error: {}", e)),
        },
        "ls" | "buffers" => {
            let buffer_list = editor.list_buffers();
            editor.set_status_message(buffer_list);
        }

        // Window/Split commands
        "split" | "sp" => {
            let message = editor.split_horizontal();
            editor.set_status_message(message);
        }
        "vsplit" | "vsp" => {
            let message = editor.split_vertical();
            editor.set_status_message(message);
        }
        "close" => {
            let message = editor.close_window();
            editor.set_status_message(message);
        }

        _ => {
            // Handle parameterized commands
            if let Some(filename) = command.strip_prefix("e ") {
                let filename = filename.trim();
                match editor.open_file(filename) {
                    Ok(msg) => editor.set_status_message(msg),
                    Err(e) => editor.set_status_message(format!("Error opening file: {}", e)),
                }
            } else if let Some(filename) = command.strip_prefix("w ") {
                let filename = filename.trim();
                match editor.write_current_buffer_to(filename) {
                    Ok(msg) => editor.set_status_message(msg),
                    Err(e) => editor.set_status_message(format!("Error writing file: {}", e)),
                }
            } else if let Some(filename) = command.strip_prefix("w! ") {
                let filename = filename.trim();
                match editor.write_current_buffer_to_force(filename) {
                    Ok(msg) => editor.set_status_message(msg),
                    Err(e) => editor.set_status_message(format!("Error writing file: {}", e)),
                }
            } else if let Some(filename) = command.strip_prefix("saveas ") {
                let filename = filename.trim();
                match editor.save_as_current_buffer(filename) {
                    Ok(msg) => editor.set_status_message(msg),
                    Err(e) => editor.set_status_message(format!("Error saving as: {}", e)),
                }
            } else if let Some(filename) = command.strip_prefix("saveas! ") {
                let filename = filename.trim();
                match editor.save_as_current_buffer_force(filename) {
                    Ok(msg) => editor.set_status_message(msg),
                    Err(e) => editor.set_status_message(format!("Error saving as: {}", e)),
                }
            } else if command == "w!" {
                if let Err(e) = editor.save_current_buffer_force() {
                    editor.set_status_message(format!("Error saving: {}", e));
                }
            } else if let Some(buffer_ref) = command.strip_prefix("b ") {
                let buffer_ref = buffer_ref.trim();
                if let Ok(buffer_id) = buffer_ref.parse::<usize>() {
                    if editor.switch_to_buffer(buffer_id) {
                        editor.set_status_message(format!("Switched to buffer {}", buffer_id));
                    } else {
                        editor.set_status_message(format!("No buffer with ID {}", buffer_id));
                    }
                } else if editor.switch_to_buffer_by_name(buffer_ref) {
                    editor.set_status_message(format!("Switched to buffer '{}'", buffer_ref));
                } else {
                    editor.set_status_message(format!("Unknown command: {}", command));
                }
            } else if let Some(set_args) = command.strip_prefix("set ") {
                handle_set_command(editor, set_args);
            } else {
                editor.set_status_message(format!("Unknown command: {}", command));
            }
        }
    }

    // Return to normal mode and clear command line
    editor.set_mode(Mode::Normal);
    editor.set_command_line(String::new());
}

/// Comprehensive :set handler (boolean toggles, key=value, queries, and \"all\").
pub fn handle_set_command(editor: &mut Editor, args: &str) {
    let args = args.trim();

    // Empty shows a subset of common settings
    if args.is_empty() {
        let mut settings = Vec::new();
        for key in [
            "number",
            "relativenumber",
            "cursorline",
            "showmarks",
            "tabstop",
            "expandtab",
            "wrap",
            "linebreak",
        ] {
            settings.push(format!(
                "{}: {}",
                key,
                editor.get_config_value(key).unwrap_or_default()
            ));
        }
        editor.set_status_message(format!("Current settings: {}", settings.join(", ")));
        return;
    }

    // set all
    if args == "all" {
        let all_settings = [
            "number",
            "relativenumber",
            "cursorline",
            "showmarks",
            "tabstop",
            "expandtab",
            "autoindent",
            "ignorecase",
            "smartcase",
            "hlsearch",
            "incsearch",
            "wrap",
            "linebreak",
            "undolevels",
            "undofile",
            "backup",
            "swapfile",
            "autosave",
            "laststatus",
            "showcmd",
            "scrolloff",
            "sidescrolloff",
            "timeoutlen",
            "percentpathroot",
            "colorscheme",
            "syntax",
        ];
        let mut all_values = Vec::new();
        for setting in &all_settings {
            if let Some(value) = editor.get_config_value(setting) {
                all_values.push(format!("{}={}", setting, value));
            }
        }
        editor.set_status_message(format!("All settings: {}", all_values.join(" | ")));
        return;
    }

    // Query e.g., set number?
    if let Some(setting) = args.strip_suffix('?') {
        if let Some(value) = editor.get_config_value(setting) {
            editor.set_status_message(format!("{}: {}", setting, value));
        } else {
            editor.set_status_message(format!("Unknown setting: {}", setting));
        }
        return;
    }

    // no<opt>
    if let Some(setting) = args.strip_prefix("no") {
        match setting {
            "number" | "nu" => {
                editor.set_config_setting("number", "false");
                let (_, relative) = editor.get_line_number_state();
                editor.set_line_numbers(false, relative);
            }
            "relativenumber" | "rnu" => {
                editor.set_config_setting("relativenumber", "false");
                let (absolute, _) = editor.get_line_number_state();
                editor.set_line_numbers(absolute, false);
            }
            "cursorline" | "cul" => {
                editor.set_config_setting("cursorline", "false");
                editor.set_cursor_line(false);
            }
            "showmarks" | "smk" => editor.set_config_setting("showmarks", "false"),
            "ignorecase" | "ic" => editor.set_config_setting("ignorecase", "false"),
            "smartcase" | "scs" => editor.set_config_setting("smartcase", "false"),
            "hlsearch" | "hls" => editor.set_config_setting("hlsearch", "false"),
            "expandtab" | "et" => editor.set_config_setting("expandtab", "false"),
            "autoindent" | "ai" => editor.set_config_setting("autoindent", "false"),
            "incsearch" | "is" => editor.set_config_setting("incsearch", "false"),
            "wrap" => editor.set_config_setting("wrap", "false"),
            "linebreak" | "lbr" => editor.set_config_setting("linebreak", "false"),
            "undofile" | "udf" => editor.set_config_setting("undofile", "false"),
            "backup" | "bk" => editor.set_config_setting("backup", "false"),
            "swapfile" | "swf" => editor.set_config_setting("swapfile", "false"),
            "autosave" | "aw" => editor.set_config_setting("autosave", "false"),
            "laststatus" | "ls" => editor.set_config_setting("laststatus", "false"),
            "showcmd" | "sc" => editor.set_config_setting("showcmd", "false"),
            "syntax" | "syn" => editor.set_config_setting("syntax", "false"),
            "percentpathroot" | "ppr" => editor.set_config_setting("percentpathroot", "false"),
            _ => editor.set_status_message(format!("Unknown option: no{}", setting)),
        }
        return;
    }

    // key=value
    if let Some((setting, value)) = args.split_once('=') {
        match setting.trim() {
            "tabstop" | "ts" => {
                if value.parse::<usize>().is_ok() {
                    editor.set_config_setting("tabstop", value);
                    editor.set_status_message(format!("Tab width set to {}", value));
                } else {
                    editor.set_status_message("Invalid tab width value".to_string());
                }
            }
            "undolevels" | "ul" => {
                if value.parse::<usize>().is_ok() {
                    editor.set_config_setting("undolevels", value);
                    editor.set_status_message(format!("Undo levels set to {}", value));
                } else {
                    editor.set_status_message("Invalid undo levels value".to_string());
                }
            }
            "scrolloff" | "so" => {
                if value.parse::<usize>().is_ok() {
                    editor.set_config_setting("scrolloff", value);
                    editor.set_status_message(format!("Scroll offset set to {}", value));
                } else {
                    editor.set_status_message("Invalid scroll offset value".to_string());
                }
            }
            "sidescrolloff" | "siso" => {
                if value.parse::<usize>().is_ok() {
                    editor.set_config_setting("sidescrolloff", value);
                    editor.set_status_message(format!("Side scroll offset set to {}", value));
                } else {
                    editor.set_status_message("Invalid side scroll offset value".to_string());
                }
            }
            "timeoutlen" | "tm" => {
                if value.parse::<u64>().is_ok() {
                    editor.set_config_setting("timeoutlen", value);
                    editor.set_status_message(format!("Command timeout set to {} ms", value));
                } else {
                    editor.set_status_message("Invalid timeout value".to_string());
                }
            }
            "colorscheme" | "colo" => {
                editor.set_config_setting("colorscheme", value);
                editor.set_status_message(format!("Color scheme set to {}", value));
            }
            "percentpathroot" | "ppr" => {
                if value.parse::<bool>().is_ok() {
                    editor.set_config_setting("percentpathroot", value);
                    editor.set_status_message(format!("Percent path root set to {}", value));
                } else {
                    editor.set_status_message("Invalid boolean value".to_string());
                }
            }
            _ => editor.set_status_message(format!("Unknown setting: {}", setting)),
        }
        return;
    }

    // Booleans enable
    match args {
        "number" | "nu" => {
            editor.set_config_setting("number", "true");
            let (_, relative) = editor.get_line_number_state();
            editor.set_line_numbers(true, relative);
        }
        "relativenumber" | "rnu" => {
            editor.set_config_setting("relativenumber", "true");
            let (absolute, _) = editor.get_line_number_state();
            editor.set_line_numbers(absolute, true);
        }
        "cursorline" | "cul" => {
            editor.set_config_setting("cursorline", "true");
            editor.set_cursor_line(true);
        }
        "showmarks" | "smk" => editor.set_config_setting("showmarks", "true"),
        "ignorecase" | "ic" => editor.set_config_setting("ignorecase", "true"),
        "smartcase" | "scs" => editor.set_config_setting("smartcase", "true"),
        "hlsearch" | "hls" => editor.set_config_setting("hlsearch", "true"),
        "expandtab" | "et" => editor.set_config_setting("expandtab", "true"),
        "autoindent" | "ai" => editor.set_config_setting("autoindent", "true"),
        "incsearch" | "is" => editor.set_config_setting("incsearch", "true"),
        "wrap" => editor.set_config_setting("wrap", "true"),
        "linebreak" | "lbr" => editor.set_config_setting("linebreak", "true"),
        "undofile" | "udf" => editor.set_config_setting("undofile", "true"),
        "backup" | "bk" => editor.set_config_setting("backup", "true"),
        "swapfile" | "swf" => editor.set_config_setting("swapfile", "true"),
        "autosave" | "aw" => editor.set_config_setting("autosave", "true"),
        "laststatus" | "ls" => editor.set_config_setting("laststatus", "true"),
        "showcmd" | "sc" => editor.set_config_setting("showcmd", "true"),
        "syntax" | "syn" => editor.set_config_setting("syntax", "true"),
        "percentpathroot" | "ppr" => editor.set_config_setting("percentpathroot", "true"),
        _ => editor.set_status_message(format!("Unknown option: {}", args)),
    }
}
