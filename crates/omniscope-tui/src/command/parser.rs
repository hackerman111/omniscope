const COMMANDS: &[&str] = &[
    "q",
    "quit",
    "qa",
    "q!",
    "w",
    "write",
    "wq",
    "add",
    "open",
    "tags",
    "help",
    "search",
    "refresh",
    "sort title",
    "sort year",
    "sort year_asc",
    "sort rating",
    "sort frecency",
    "sort updated",
    "lib",
    "library",
    "tag",
    "marks",
    "reg",
    "registers",
    "delmarks",
    "doctor",
    "macros",
    "copen",
    "cclose",
    "cnext",
    "cprev",
    "cn",
    "cp",
    "undolist",
    "earlier",
    "later",
    "sync",
];

pub fn get_command_suggestions(prefix: &str) -> Vec<&'static str> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return COMMANDS.iter().take(10).copied().collect();
    }
    COMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(prefix))
        .take(10)
        .copied()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Quit,
    Write,
    WriteQuit,
    Add,
    Open,
    Tags,
    Help,
    Search(String),
    Global {
        pattern: String,
        command: String,
    },
    Substitute {
        pattern: String,
        replacement: String,
        global: bool,
    },
    Refresh,
    UndoList,
    Earlier(String),
    Later(String),
    QuickfixOpen,
    QuickfixClose,
    QuickfixDo(String),
    QuickfixNext,
    QuickfixPrev,
    Sort(String),
    Library(String),
    FilterTag(String),
    Marks,
    Registers(Option<char>),
    DeleteMarks(String),
    Doctor,
    Macros,
    Sync,
    Unknown(String),
}

/// Parse a raw command-line string into a CommandAction.
pub fn parse_command(cmd: &str) -> CommandAction {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return CommandAction::Unknown("".to_string());
    }

    match parts.as_slice() {
        ["q"] | ["quit"] | ["qa"] | ["q!"] => CommandAction::Quit,
        ["w"] | ["write"] => CommandAction::Write,
        ["wq"] => CommandAction::WriteQuit,
        ["add"] => CommandAction::Add,
        ["open"] => CommandAction::Open,
        ["tags"] => CommandAction::Tags,
        ["help"] => CommandAction::Help,
        ["search" | "find", rest @ ..] => CommandAction::Search(rest.join(" ")),
        ["refresh"] => CommandAction::Refresh,
        ["undolist"] => CommandAction::UndoList,
        ["earlier", time] => CommandAction::Earlier(time.to_string()),
        ["later", time] => CommandAction::Later(time.to_string()),
        ["copen"] => CommandAction::QuickfixOpen,
        ["cclose"] => CommandAction::QuickfixClose,
        ["cnext"] | ["cn"] => CommandAction::QuickfixNext,
        ["cprev"] | ["cp"] => CommandAction::QuickfixPrev,
        ["cdo", rest @ ..] => CommandAction::QuickfixDo(rest.join(" ")),
        // New commands
        ["sort", field, ..] => CommandAction::Sort(field.to_string()),
        ["lib" | "library", name, ..] => CommandAction::Library(name.to_string()),
        ["tag", name, ..] => CommandAction::FilterTag(name.to_string()),
        ["marks"] => CommandAction::Marks,
        ["reg" | "registers"] => CommandAction::Registers(None),
        ["reg" | "registers", r] => {
            let ch = r.chars().next();
            CommandAction::Registers(ch)
        }
        ["delmarks", marks] => CommandAction::DeleteMarks(marks.to_string()),
        ["doctor"] => CommandAction::Doctor,
        ["macros"] => CommandAction::Macros,
        ["sync"] => CommandAction::Sync,
        ["tabnew", ..] => {
            // Tabs not implemented yet, but parse gracefully
            CommandAction::Unknown("tabnew (tabs not implemented)".to_string())
        }
        ["bnext" | "bn"] => CommandAction::Unknown("bnext (buffers not implemented)".to_string()),
        ["bprev" | "bp"] => CommandAction::Unknown("bprev (buffers not implemented)".to_string()),
        _ => {
            let cmd_str = parts.join(" ");

            // Check for :g/pattern/command
            if cmd_str.starts_with("g/") || cmd_str.starts_with("v/") {
                let parts: Vec<&str> = cmd_str.splitn(3, '/').collect();
                if parts.len() >= 3 {
                    let pattern = parts[1].to_string();
                    let command = parts[2].to_string();
                    return CommandAction::Global { pattern, command };
                }
            }

            // Check for :%s/foo/bar/ or :s/foo/bar/g
            if cmd_str.starts_with("%s/") || cmd_str.starts_with("s/") {
                let parts: Vec<&str> = cmd_str.split('/').collect();
                if parts.len() >= 3 {
                    let pattern = parts[1].to_string();
                    let replacement = parts[2].to_string();
                    let global = parts.get(3).map_or(false, |&f| f.contains('g'));
                    return CommandAction::Substitute {
                        pattern,
                        replacement,
                        global,
                    };
                }
            }

            CommandAction::Unknown(cmd.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_commands() {
        assert_eq!(parse_command("q"), CommandAction::Quit);
        assert_eq!(parse_command("quit"), CommandAction::Quit);
        assert_eq!(parse_command("q!"), CommandAction::Quit);
        assert_eq!(parse_command("qa"), CommandAction::Quit);
        assert_eq!(parse_command("w"), CommandAction::Write);
        assert_eq!(parse_command("wq"), CommandAction::WriteQuit);
    }

    #[test]
    fn test_sort_command() {
        assert_eq!(
            parse_command("sort title"),
            CommandAction::Sort("title".to_string())
        );
        assert_eq!(
            parse_command("sort year"),
            CommandAction::Sort("year".to_string())
        );
    }

    #[test]
    fn test_library_command() {
        assert_eq!(
            parse_command("lib fiction"),
            CommandAction::Library("fiction".to_string())
        );
    }

    #[test]
    fn test_marks_registers() {
        assert_eq!(parse_command("marks"), CommandAction::Marks);
        assert_eq!(parse_command("reg"), CommandAction::Registers(None));
        assert_eq!(parse_command("reg a"), CommandAction::Registers(Some('a')));
        assert_eq!(
            parse_command("delmarks abc"),
            CommandAction::DeleteMarks("abc".to_string())
        );
    }

    #[test]
    fn test_doctor() {
        assert_eq!(parse_command("doctor"), CommandAction::Doctor);
    }

    #[test]
    fn test_macros() {
        assert_eq!(parse_command("macros"), CommandAction::Macros);
    }

    #[test]
    fn test_global_command() {
        assert_eq!(
            parse_command("g/rust/d"),
            CommandAction::Global {
                pattern: "rust".to_string(),
                command: "d".to_string()
            }
        );
    }

    #[test]
    fn test_substitute() {
        match parse_command("s/foo/bar/g") {
            CommandAction::Substitute {
                pattern,
                replacement,
                global,
            } => {
                assert_eq!(pattern, "foo");
                assert_eq!(replacement, "bar");
                assert!(global);
            }
            other => panic!("Expected Substitute, got {:?}", other),
        }
    }
}
