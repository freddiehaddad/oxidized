//! Completion schema: canonicalization and kinds.

/// Map a :set alias or key to its canonical long name.
/// Examples: ts -> tabstop, rnu -> relativenumber
pub fn alias_to_canonical(key: &str) -> &str {
    match key {
        "nu" => "number",
        "rnu" => "relativenumber",
        "cul" => "cursorline",
        "smk" => "showmarks",
        "et" => "expandtab",
        "ai" => "autoindent",
        "ic" => "ignorecase",
        "scs" => "smartcase",
        "hls" => "hlsearch",
        "is" => "incsearch",
        "lbr" => "linebreak",
        "udf" => "undofile",
        "bk" => "backup",
        "swf" => "swapfile",
        "aw" => "autosave",
        "ls" => "laststatus",
        "sc" => "showcmd",
        "ppr" => "percentpathroot",
        "syn" => "syntax",
        "ts" => "tabstop",
        "ul" => "undolevels",
        "so" => "scrolloff",
        "siso" => "sidescrolloff",
        "tm" => "timeoutlen",
        "colo" => "colorscheme",
        other => other,
    }
}
