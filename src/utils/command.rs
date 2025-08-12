// Command parsing and execution
// This handles Ex-style commands (:w, :q, etc.) in a central place.

use log::{debug, error, info, trace, warn};

use crate::core::editor::Editor;
use crate::core::mode::Mode;

pub struct Command {
    pub name: String,
    pub args: Vec<String>,
}

impl Command {
    pub fn parse(input: &str) -> Option<Self> {
        // Parsing logs are very chatty; keep at trace level
        trace!("Parsing command: '{}'", input);
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            // An empty parse is user-initiated and expected sometimes; keep at trace
            trace!("Empty command input received");
            return None;
        }

        let name = parts[0].to_string();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        trace!(
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
    debug!("Executing Ex command: '{}'", command);

    match command {
        // Quit commands
        "q" | "quit" => editor.quit(),
        "q!" | "quit!" => editor.force_quit(),

        // Write/save commands
        "w" | "write" => {
            if let Err(e) = editor.save_current_buffer() {
                error!("Save failed: {}", e);
                editor.set_status_message(format!("Error saving: {}", e));
            } else {
                info!("Buffer saved successfully");
            }
        }

        // Write and quit
        "wq" | "x" => {
            if let Err(e) = editor.save_current_buffer() {
                error!("Save before quit failed: {}", e);
                editor.set_status_message(format!("Error saving: {}", e));
            } else {
                info!("Saved buffer; quitting editor");
                editor.quit();
            }
        }
        // Force write and quit
        "wq!" | "x!" => {
            if let Err(e) = editor.save_current_buffer_force() {
                error!("Force save before quit failed: {}", e);
                editor.set_status_message(format!("Error saving: {}", e));
            } else {
                info!("Force-saved buffer; quitting editor");
                editor.quit();
            }
        }

        // Create a new empty buffer and switch to it
        "enew" => match editor.create_buffer(None) {
            Ok(id) => {
                info!("Created new empty buffer {}", id);
                editor.set_status_message(format!("New empty buffer {}", id))
            }
            Err(e) => editor.set_status_message(format!("Error creating buffer: {}", e)),
        },

        // Line number toggles (ephemeral; use :setp for persistence)
        "set nu" | "set number" => editor.set_line_numbers_ephemeral(true, false),
        "set nonu" | "set nonumber" => editor.set_line_numbers_ephemeral(false, false),
        "set rnu" | "set relativenumber" => editor.set_line_numbers_ephemeral(false, true),
        "set nornu" | "set norelativenumber" => editor.set_line_numbers_ephemeral(true, false),
        "set nu rnu" | "set number relativenumber" => editor.set_line_numbers_ephemeral(true, true),

        // Cursor line toggles (ephemeral; use :setp cursorline / :setp nocursorline for persistence)
        "set cul" | "set cursorline" => editor.set_cursor_line_ephemeral(true),
        "set nocul" | "set nocursorline" => editor.set_cursor_line_ephemeral(false),

        // Buffer management
        "bn" | "bnext" => {
            if editor.switch_to_next_buffer() {
                editor.set_status_message("Switched to next buffer".to_string());
            } else {
                editor.set_status_message("No next buffer".to_string());
            }
        }
        "bd" | "bdelete" => match editor.close_current_buffer() {
            Ok(msg) => {
                info!("Closed current buffer");
                editor.set_status_message(msg)
            }
            Err(e) => editor.set_status_message(format!("Error: {}", e)),
        },
        "bd!" | "bdelete!" => match editor.force_close_current_buffer() {
            Ok(msg) => {
                warn!("Force-closed current buffer");
                editor.set_status_message(msg)
            }
            Err(e) => editor.set_status_message(format!("Error: {}", e)),
        },
        "ls" | "buffers" => {
            let buffer_list = editor.list_buffers();
            editor.set_status_message(buffer_list);
        }

        // Window/Split commands
        "split" | "sp" => {
            let message = editor.split_horizontal();
            info!("Created horizontal split");
            editor.set_status_message(message);
        }
        "vsplit" | "vsp" => {
            let message = editor.split_vertical();
            info!("Created vertical split");
            editor.set_status_message(message);
        }
        "close" => {
            let message = editor.close_window();
            info!("Closed window");
            editor.set_status_message(message);
        }

        _ => {
            // Handle parameterized commands
            if let Some(filename) = command.strip_prefix("e ") {
                let filename = filename.trim();
                match editor.open_file(filename) {
                    Ok(msg) => {
                        info!("Opened file '{}'", filename);
                        editor.set_status_message(msg)
                    }
                    Err(e) => {
                        error!("Open file '{}' failed: {}", filename, e);
                        editor.set_status_message(format!("Error opening file: {}", e))
                    }
                }
            } else if let Some(filename) = command.strip_prefix("w ") {
                let filename = filename.trim();
                match editor.write_current_buffer_to(filename) {
                    Ok(msg) => {
                        info!("Wrote current buffer to '{}'", filename);
                        editor.set_status_message(msg)
                    }
                    Err(e) => {
                        error!("Write to '{}' failed: {}", filename, e);
                        editor.set_status_message(format!("Error writing file: {}", e))
                    }
                }
            } else if let Some(filename) = command.strip_prefix("w! ") {
                let filename = filename.trim();
                match editor.write_current_buffer_to_force(filename) {
                    Ok(msg) => {
                        warn!("Force-wrote current buffer to '{}'", filename);
                        editor.set_status_message(msg)
                    }
                    Err(e) => {
                        error!("Force write to '{}' failed: {}", filename, e);
                        editor.set_status_message(format!("Error writing file: {}", e))
                    }
                }
            } else if let Some(filename) = command.strip_prefix("saveas ") {
                let filename = filename.trim();
                match editor.save_as_current_buffer(filename) {
                    Ok(msg) => {
                        info!("Saved as '{}'", filename);
                        editor.set_status_message(msg)
                    }
                    Err(e) => {
                        error!("Save-as '{}' failed: {}", filename, e);
                        editor.set_status_message(format!("Error saving as: {}", e))
                    }
                }
            } else if let Some(filename) = command.strip_prefix("saveas! ") {
                let filename = filename.trim();
                match editor.save_as_current_buffer_force(filename) {
                    Ok(msg) => {
                        warn!("Force saved-as '{}'", filename);
                        editor.set_status_message(msg)
                    }
                    Err(e) => {
                        error!("Force save-as '{}' failed: {}", filename, e);
                        editor.set_status_message(format!("Error saving as: {}", e))
                    }
                }
            } else if command == "w!" {
                if let Err(e) = editor.save_current_buffer_force() {
                    error!("Force save failed: {}", e);
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
                    warn!("Unknown buffer reference in command: '{}'", command);
                    editor.set_status_message(format!("Unknown command: {}", command));
                }
            } else if let Some(set_args) = command.strip_prefix("setp ") {
                // Persistent set (writes to editor.toml)
                handle_set_command(editor, set_args, true);
            } else if let Some(set_args) = command.strip_prefix("set ") {
                // Ephemeral set (does not persist)
                handle_set_command(editor, set_args, false);
            } else {
                warn!("Unknown Ex command: '{}'", command);
                editor.set_status_message(format!("Unknown command: {}", command));
            }
        }
    }

    // Return to normal mode and clear command line
    editor.set_mode(Mode::Normal);
    editor.set_command_line(String::new());
}

/// Comprehensive :set handler (boolean toggles, key=value, queries, and \"all\").
pub fn handle_set_command(editor: &mut Editor, args: &str, persist: bool) {
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

    // Positional colorscheme (e.g., "set colorscheme default" or "set colo default")
    if let Some(rest) = args
        .strip_prefix("colorscheme ")
        .or_else(|| args.strip_prefix("colo "))
    {
        let value = rest.trim();
        if !value.is_empty() {
            if persist {
                editor.set_config_setting("colorscheme", value);
            } else {
                editor.set_config_setting_ephemeral("colorscheme", value);
            }
            editor.set_status_message(format!("Color scheme set to {}", value));
            return;
        }
    }

    // no<opt>
    if let Some(setting) = args.strip_prefix("no") {
        match setting {
            "number" | "nu" => {
                if persist {
                    editor.set_config_setting("number", "false");
                } else {
                    editor.set_config_setting_ephemeral("number", "false");
                }
                let (_, relative) = editor.get_line_number_state();
                if persist {
                    editor.set_line_numbers(false, relative);
                } else {
                    editor.set_line_numbers_ephemeral(false, relative);
                }
            }
            "relativenumber" | "rnu" => {
                if persist {
                    editor.set_config_setting("relativenumber", "false");
                } else {
                    editor.set_config_setting_ephemeral("relativenumber", "false");
                }
                let (absolute, _) = editor.get_line_number_state();
                if persist {
                    editor.set_line_numbers(absolute, false);
                } else {
                    editor.set_line_numbers_ephemeral(absolute, false);
                }
            }
            "cursorline" | "cul" => {
                if persist {
                    editor.set_config_setting("cursorline", "false");
                    editor.set_cursor_line(false);
                } else {
                    editor.set_config_setting_ephemeral("cursorline", "false");
                    editor.set_cursor_line_ephemeral(false);
                }
            }
            "showmarks" | "smk" => {
                if persist {
                    editor.set_config_setting("showmarks", "false")
                } else {
                    editor.set_config_setting_ephemeral("showmarks", "false")
                }
            }
            "ignorecase" | "ic" => {
                if persist {
                    editor.set_config_setting("ignorecase", "false")
                } else {
                    editor.set_config_setting_ephemeral("ignorecase", "false")
                }
            }
            "smartcase" | "scs" => {
                if persist {
                    editor.set_config_setting("smartcase", "false")
                } else {
                    editor.set_config_setting_ephemeral("smartcase", "false")
                }
            }
            "hlsearch" | "hls" => {
                if persist {
                    editor.set_config_setting("hlsearch", "false")
                } else {
                    editor.set_config_setting_ephemeral("hlsearch", "false")
                }
            }
            "expandtab" | "et" => {
                if persist {
                    editor.set_config_setting("expandtab", "false")
                } else {
                    editor.set_config_setting_ephemeral("expandtab", "false")
                }
            }
            "autoindent" | "ai" => {
                if persist {
                    editor.set_config_setting("autoindent", "false")
                } else {
                    editor.set_config_setting_ephemeral("autoindent", "false")
                }
            }
            "incsearch" | "is" => {
                if persist {
                    editor.set_config_setting("incsearch", "false")
                } else {
                    editor.set_config_setting_ephemeral("incsearch", "false")
                }
            }
            "wrap" => {
                if persist {
                    editor.set_config_setting("wrap", "false")
                } else {
                    editor.set_config_setting_ephemeral("wrap", "false")
                }
            }
            "linebreak" | "lbr" => {
                if persist {
                    editor.set_config_setting("linebreak", "false")
                } else {
                    editor.set_config_setting_ephemeral("linebreak", "false")
                }
            }
            "undofile" | "udf" => {
                if persist {
                    editor.set_config_setting("undofile", "false")
                } else {
                    editor.set_config_setting_ephemeral("undofile", "false")
                }
            }
            "backup" | "bk" => {
                if persist {
                    editor.set_config_setting("backup", "false")
                } else {
                    editor.set_config_setting_ephemeral("backup", "false")
                }
            }
            "swapfile" | "swf" => {
                if persist {
                    editor.set_config_setting("swapfile", "false")
                } else {
                    editor.set_config_setting_ephemeral("swapfile", "false")
                }
            }
            "autosave" | "aw" => {
                if persist {
                    editor.set_config_setting("autosave", "false")
                } else {
                    editor.set_config_setting_ephemeral("autosave", "false")
                }
            }
            "laststatus" | "ls" => {
                if persist {
                    editor.set_config_setting("laststatus", "false")
                } else {
                    editor.set_config_setting_ephemeral("laststatus", "false")
                }
            }
            "showcmd" | "sc" => {
                if persist {
                    editor.set_config_setting("showcmd", "false")
                } else {
                    editor.set_config_setting_ephemeral("showcmd", "false")
                }
            }
            "syntax" | "syn" => {
                if persist {
                    editor.set_config_setting("syntax", "false")
                } else {
                    editor.set_config_setting_ephemeral("syntax", "false")
                }
            }
            "percentpathroot" | "ppr" => {
                if persist {
                    editor.set_config_setting("percentpathroot", "false")
                } else {
                    editor.set_config_setting_ephemeral("percentpathroot", "false")
                }
            }
            _ => {
                warn!("Unknown :set option: no{}", setting);
                editor.set_status_message(format!("Unknown option: no{}", setting))
            }
        }
        return;
    }

    // Positional numeric/argument settings
    if let Some(value) = args
        .strip_prefix("tabstop ")
        .or_else(|| args.strip_prefix("ts "))
    {
        let val = value.trim();
        if val.parse::<usize>().is_ok() {
            if persist {
                editor.set_config_setting("tabstop", val);
            } else {
                editor.set_config_setting_ephemeral("tabstop", val);
            }
            editor.set_status_message(format!("Tab width set to {}", val));
        } else {
            editor.set_status_message("Invalid tab width value".to_string());
        }
        return;
    }
    if let Some(value) = args
        .strip_prefix("undolevels ")
        .or_else(|| args.strip_prefix("ul "))
    {
        let val = value.trim();
        if val.parse::<usize>().is_ok() {
            if persist {
                editor.set_config_setting("undolevels", val);
            } else {
                editor.set_config_setting_ephemeral("undolevels", val);
            }
            editor.set_status_message(format!("Undo levels set to {}", val));
        } else {
            editor.set_status_message("Invalid undo levels value".to_string());
        }
        return;
    }
    if let Some(value) = args
        .strip_prefix("scrolloff ")
        .or_else(|| args.strip_prefix("so "))
    {
        let val = value.trim();
        if val.parse::<usize>().is_ok() {
            if persist {
                editor.set_config_setting("scrolloff", val);
            } else {
                editor.set_config_setting_ephemeral("scrolloff", val);
            }
            editor.set_status_message(format!("Scroll offset set to {}", val));
        } else {
            editor.set_status_message("Invalid scroll offset value".to_string());
        }
        return;
    }
    if let Some(value) = args
        .strip_prefix("sidescrolloff ")
        .or_else(|| args.strip_prefix("siso "))
    {
        let val = value.trim();
        if val.parse::<usize>().is_ok() {
            if persist {
                editor.set_config_setting("sidescrolloff", val);
            } else {
                editor.set_config_setting_ephemeral("sidescrolloff", val);
            }
            editor.set_status_message(format!("Side scroll offset set to {}", val));
        } else {
            editor.set_status_message("Invalid side scroll offset value".to_string());
        }
        return;
    }
    if let Some(value) = args
        .strip_prefix("timeoutlen ")
        .or_else(|| args.strip_prefix("tm "))
    {
        let val = value.trim();
        if val.parse::<u64>().is_ok() {
            if persist {
                editor.set_config_setting("timeoutlen", val);
            } else {
                editor.set_config_setting_ephemeral("timeoutlen", val);
            }
            editor.set_status_message(format!("Command timeout set to {} ms", val));
        } else {
            editor.set_status_message("Invalid timeout value".to_string());
        }
        return;
    }
    if let Some(value) = args
        .strip_prefix("percentpathroot ")
        .or_else(|| args.strip_prefix("ppr "))
    {
        let val = value.trim();
        if val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("false") {
            if persist {
                editor.set_config_setting("percentpathroot", val);
            } else {
                editor.set_config_setting_ephemeral("percentpathroot", val);
            }
            editor.set_status_message(format!("Percent path root set to {}", val));
        } else {
            editor.set_status_message("Invalid boolean value".to_string());
        }
        return;
    }

    // Booleans enable
    match args {
        "number" | "nu" => {
            if persist {
                editor.set_config_setting("number", "true");
            } else {
                editor.set_config_setting_ephemeral("number", "true");
            }
            let (_, relative) = editor.get_line_number_state();
            if persist {
                editor.set_line_numbers(true, relative);
            } else {
                editor.set_line_numbers_ephemeral(true, relative);
            }
        }
        "relativenumber" | "rnu" => {
            if persist {
                editor.set_config_setting("relativenumber", "true");
            } else {
                editor.set_config_setting_ephemeral("relativenumber", "true");
            }
            let (absolute, _) = editor.get_line_number_state();
            if persist {
                editor.set_line_numbers(absolute, true);
            } else {
                editor.set_line_numbers_ephemeral(absolute, true);
            }
        }
        "cursorline" | "cul" => {
            if persist {
                editor.set_config_setting("cursorline", "true");
                editor.set_cursor_line(true);
            } else {
                editor.set_config_setting_ephemeral("cursorline", "true");
                editor.set_cursor_line_ephemeral(true);
            }
        }
        "showmarks" | "smk" => {
            if persist {
                editor.set_config_setting("showmarks", "true")
            } else {
                editor.set_config_setting_ephemeral("showmarks", "true")
            }
        }
        "ignorecase" | "ic" => {
            if persist {
                editor.set_config_setting("ignorecase", "true")
            } else {
                editor.set_config_setting_ephemeral("ignorecase", "true")
            }
        }
        "smartcase" | "scs" => {
            if persist {
                editor.set_config_setting("smartcase", "true")
            } else {
                editor.set_config_setting_ephemeral("smartcase", "true")
            }
        }
        "hlsearch" | "hls" => {
            if persist {
                editor.set_config_setting("hlsearch", "true")
            } else {
                editor.set_config_setting_ephemeral("hlsearch", "true")
            }
        }
        "expandtab" | "et" => {
            if persist {
                editor.set_config_setting("expandtab", "true")
            } else {
                editor.set_config_setting_ephemeral("expandtab", "true")
            }
        }
        "autoindent" | "ai" => {
            if persist {
                editor.set_config_setting("autoindent", "true")
            } else {
                editor.set_config_setting_ephemeral("autoindent", "true")
            }
        }
        "incsearch" | "is" => {
            if persist {
                editor.set_config_setting("incsearch", "true")
            } else {
                editor.set_config_setting_ephemeral("incsearch", "true")
            }
        }
        "wrap" => {
            if persist {
                editor.set_config_setting("wrap", "true")
            } else {
                editor.set_config_setting_ephemeral("wrap", "true")
            }
        }
        "linebreak" | "lbr" => {
            if persist {
                editor.set_config_setting("linebreak", "true")
            } else {
                editor.set_config_setting_ephemeral("linebreak", "true")
            }
        }
        "undofile" | "udf" => {
            if persist {
                editor.set_config_setting("undofile", "true")
            } else {
                editor.set_config_setting_ephemeral("undofile", "true")
            }
        }
        "backup" | "bk" => {
            if persist {
                editor.set_config_setting("backup", "true")
            } else {
                editor.set_config_setting_ephemeral("backup", "true")
            }
        }
        "swapfile" | "swf" => {
            if persist {
                editor.set_config_setting("swapfile", "true")
            } else {
                editor.set_config_setting_ephemeral("swapfile", "true")
            }
        }
        "autosave" | "aw" => {
            if persist {
                editor.set_config_setting("autosave", "true")
            } else {
                editor.set_config_setting_ephemeral("autosave", "true")
            }
        }
        "laststatus" | "ls" => {
            if persist {
                editor.set_config_setting("laststatus", "true")
            } else {
                editor.set_config_setting_ephemeral("laststatus", "true")
            }
        }
        "showcmd" | "sc" => {
            if persist {
                editor.set_config_setting("showcmd", "true")
            } else {
                editor.set_config_setting_ephemeral("showcmd", "true")
            }
        }
        "syntax" | "syn" => {
            if persist {
                editor.set_config_setting("syntax", "true")
            } else {
                editor.set_config_setting_ephemeral("syntax", "true")
            }
        }
        "percentpathroot" | "ppr" => {
            if persist {
                editor.set_config_setting("percentpathroot", "true")
            } else {
                editor.set_config_setting_ephemeral("percentpathroot", "true")
            }
        }
        _ => {
            warn!("Unknown :set option: {}", args);
            editor.set_status_message(format!("Unknown option: {}", args))
        }
    }
}
