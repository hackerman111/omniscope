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
    Unknown(String),
}

/// Parse a raw command-line string into a CommandAction.
pub fn parse_command(cmd: &str) -> CommandAction {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return CommandAction::Unknown("".to_string());
    }

    match parts.as_slice() {
        ["q"] | ["quit"] => CommandAction::Quit,
        ["w"] | ["write"] => CommandAction::Write,
        ["wq"] => CommandAction::WriteQuit,
        ["add"] => CommandAction::Add,
        ["open"] => CommandAction::Open,
        ["tags"] => CommandAction::Tags,
        ["help"] => CommandAction::Help,
        ["search" | "find", rest @ ..] => {
            CommandAction::Search(rest.join(" "))
        }
        ["refresh"] => CommandAction::Refresh,
        ["undolist"] => CommandAction::UndoList,
        ["earlier", time] => CommandAction::Earlier(time.to_string()),
        ["later", time] => CommandAction::Later(time.to_string()),
        ["copen"] => CommandAction::QuickfixOpen,
        ["cclose"] => CommandAction::QuickfixClose,
        ["cnext"] | ["cn"] => CommandAction::QuickfixNext,
        ["cprev"] | ["cp"] => CommandAction::QuickfixPrev,
        ["cdo", rest @ ..] => CommandAction::QuickfixDo(rest.join(" ")),
        _ => {
            let cmd_str = parts.join(" ");
            
            // Check for :g/pattern/command
            if cmd_str.starts_with("g/") || cmd_str.starts_with("v/") {
                // simple parsing: g/pattern/command
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
                 if parts.len() >= 3 { // %s, pattern, replacement, [flags]
                     let pattern = parts[1].to_string();
                     let replacement = parts[2].to_string();
                     let global = parts.get(3).map_or(false, |&f| f.contains('g'));
                     return CommandAction::Substitute { pattern, replacement, global };
                 }
            }
            
            CommandAction::Unknown(cmd.to_string())
        }
    }
}
