use crate::app::{App, Mode};
use crate::keys::operator::Operator;

pub struct KeyHint {
    pub key: &'static str,
    pub desc: &'static str,
}

pub fn get_hints(app: &App) -> Vec<KeyHint> {
    // 1. If in Register selection mode
    if app.pending_register_select {
        return vec![
            KeyHint { key: "0-9", desc: "numbered" },
            KeyHint { key: "a-z", desc: "named" },
            KeyHint { key: "+",   desc: "sys clipboard" },
            KeyHint { key: "*",   desc: "sys selection" },
            KeyHint { key: "_",   desc: "black hole" },
        ];
    }

    // 2. If macro is recording
    if app.macro_recorder.is_recording() {
        if let Some(_reg) = app.macro_recorder.recording_register {
            return vec![
                KeyHint { key: "q", desc: &"stop recording" },
                KeyHint { key: "", desc: &"" }, // placeholder
            ];
            // Note: We return a simple hint. The status bar shows @reg recording.
        }
    }

    // 3. If an operator is pending (e.g. after 'd', 'y')
    if let Some(op) = app.pending_operator {
        let mut hints = motion_hints();
        
        // Context-specific additions for operators
        if op == Operator::Change {
            hints.insert(0, KeyHint { key: "a", desc: "author" });
            hints.insert(1, KeyHint { key: "t", desc: "tags" });
            hints.insert(2, KeyHint { key: "r", desc: "rating" });
            hints.insert(3, KeyHint { key: "s", desc: "status" });
            hints.insert(4, KeyHint { key: "y", desc: "year" });
            hints.insert(5, KeyHint { key: "n", desc: "notes" });
        }
        
        // Add text objects
        hints.extend(vec![
            KeyHint { key: "ib", desc: "inner book" },
            KeyHint { key: "ab", desc: "a book" },
            KeyHint { key: "il", desc: "inner library" },
            KeyHint { key: "al", desc: "a library" },
            KeyHint { key: "it", desc: "inner tag" },
            KeyHint { key: "at", desc: "a tag" },
            KeyHint { key: "ia", desc: "inner author" },
            KeyHint { key: "aa", desc: "a author" },
            KeyHint { key: "iy", desc: "inner year" },
            KeyHint { key: "if", desc: "inner folder" },
        ]);
        
        return hints;
    }

    // 4. Mode-specific hints
    match app.mode {
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
            return vec![
                KeyHint { key: "y", desc: "yank" },
                KeyHint { key: "d", desc: "delete" },
                KeyHint { key: "x", desc: "delete" },
                KeyHint { key: "c", desc: "change" },
                KeyHint { key: "o", desc: "swap anchor" },
                KeyHint { key: "Space", desc: "toggle" },
                KeyHint { key: "C-a", desc: "select all" },
                KeyHint { key: "C-q", desc: "quickfix" },
            ];
        }
        _ => {}
    }

    // 5. Pending key prefixes (g, z, S, etc.)
    if let Some(pending) = app.pending_key {
        match pending {
            'S' => return vec![
                KeyHint { key: "y", desc: "year desc" },
                KeyHint { key: "Y", desc: "year asc" },
                KeyHint { key: "t", desc: "title asc" },
                KeyHint { key: "r", desc: "rating desc" },
                KeyHint { key: "f", desc: "frecency" },
                KeyHint { key: "u", desc: "updated (default)" },
            ],
            'g' => return vec![
                KeyHint { key: "g", desc: "top" },
                KeyHint { key: "h", desc: "home (all)" },
                KeyHint { key: "l", desc: "last jump" },
                KeyHint { key: "p", desc: "parent" },
                KeyHint { key: "r", desc: "root" },
                KeyHint { key: "s", desc: "cycle status" },
                KeyHint { key: "t", desc: "edit title" },
                KeyHint { key: "f", desc: "open file" },
                KeyHint { key: "I", desc: "open in $EDITOR" },
                KeyHint { key: "v", desc: "reselect visual" },
                KeyHint { key: "z", desc: "center view" },
                KeyHint { key: "*", desc: "search author" },
                KeyHint { key: "b", desc: "buffers" },
                KeyHint { key: "B", desc: "prev buffer" },
            ],
            'z' => return vec![
                KeyHint { key: "z", desc: "center" },
                KeyHint { key: "t", desc: "top" },
                KeyHint { key: "b", desc: "bottom" },
                KeyHint { key: "a", desc: "toggle fold" },
                KeyHint { key: "o", desc: "open fold" },
                KeyHint { key: "c", desc: "close fold" },
                KeyHint { key: "R", desc: "open all" },
                KeyHint { key: "M", desc: "close all" },
            ],
            'm' => return vec![
                KeyHint { key: "a-z", desc: "set local mark" },
                KeyHint { key: "A-Z", desc: "set global mark" },
            ],
            '\'' => return vec![
                KeyHint { key: "a-z", desc: "jump to mark" },
                KeyHint { key: "'", desc: "last position" },
                KeyHint { key: "<", desc: "visual start" },
                KeyHint { key: ">", desc: "visual end" },
            ],
            '[' => return vec![
                KeyHint { key: "[", desc: "prev group" },
            ],
            ']' => return vec![
                KeyHint { key: "]", desc: "next group" },
            ],
            ' ' => return vec![
                KeyHint { key: "Space", desc: "all labels" },
                KeyHint { key: "j", desc: "labels below" },
                KeyHint { key: "k", desc: "labels above" },
                KeyHint { key: "/", desc: "by first letter" },
            ],
            'f' | 'F' | 't' | 'T' => return vec![
                KeyHint { key: "<char>", desc: "jump to char" },
            ],
            'Q' => return vec![
                KeyHint { key: "a-z", desc: "register to record" },
            ],
            '@' => return vec![
                KeyHint { key: "a-z", desc: "play macro" },
                KeyHint { key: "@", desc: "replay last" },
            ],
            _ => {}
        }
    }

    Vec::new()
}

fn motion_hints() -> Vec<KeyHint> {
    vec![
        KeyHint { key: "j", desc: "down" },
        KeyHint { key: "k", desc: "up" },
        KeyHint { key: "G", desc: "bottom" },
        KeyHint { key: "gg", desc: "top" },
        KeyHint { key: "0", desc: "first" },
        KeyHint { key: "$", desc: "last" },
        KeyHint { key: "f<c>", desc: "find char" },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use omniscope_core::AppConfig;
    
    fn mock_app() -> App {
        App::new(AppConfig::default())
    }

    #[test]
    fn test_visual_hints() {
        let mut app = mock_app();
        app.mode = Mode::Visual;
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "y"));
        assert!(hints.iter().any(|h| h.key == "d"));
        assert!(hints.iter().any(|h| h.key == "o"));
    }

    #[test]
    fn test_operator_hints() {
        let mut app = mock_app();
        app.pending_operator = Some(Operator::Delete);
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "j")); // Motion
        assert!(hints.iter().any(|h| h.key == "ib")); // Text object
    }

    #[test]
    fn test_change_op_hints() {
        let mut app = mock_app();
        app.pending_operator = Some(Operator::Change);
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "a" && h.desc == "author"));
        assert!(hints.iter().any(|h| h.key == "s" && h.desc == "status"));
        assert!(hints.iter().any(|h| h.key == "y" && h.desc == "year"));
    }

    #[test]
    fn test_register_hints() {
        let mut app = mock_app();
        app.pending_register_select = true;
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "+"));
    }

    #[test]
    fn test_space_hints() {
        let mut app = mock_app();
        app.pending_key = Some(' ');
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "/"));
    }

    #[test]
    fn test_z_hints() {
        let mut app = mock_app();
        app.pending_key = Some('z');
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "a")); // toggle fold
        assert!(hints.iter().any(|h| h.key == "R")); // open all
    }
}
