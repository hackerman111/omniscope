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

    // 2. If an operator is pending (e.g. after 'd', 'y')
    if let Some(op) = app.pending_operator {
        let mut hints = motion_hints();
        
        // Context-specific additions for operators
        if op == Operator::Change {
            hints.insert(0, KeyHint { key: "a", desc: "author" });
            hints.insert(1, KeyHint { key: "t", desc: "tags" });
            hints.insert(2, KeyHint { key: "r", desc: "rating" });
        }
        
        // Add text objects
        hints.extend(vec![
            KeyHint { key: "iw", desc: "inner word" },
            KeyHint { key: "aw", desc: "a word" },
        ]);
        
        return hints;
    }

    // 3. Mode-specific hints
    match app.mode {
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
            return vec![
                KeyHint { key: "y", desc: "yank" },
                KeyHint { key: "d", desc: "delete" },
                KeyHint { key: "x", desc: "delete" },
                KeyHint { key: "c", desc: "change" },
                KeyHint { key: "r", desc: "set rating" },
                KeyHint { key: "~", desc: "toggle case" },
            ];
        }
        _ => {}
    }

    // 4. Pending key prefixes (g, z, S, etc.)
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
            ],
            'z' => return vec![
                KeyHint { key: "z", desc: "center" },
                KeyHint { key: "t", desc: "top" },
                KeyHint { key: "b", desc: "bottom" },
            ],
            'm' => return vec![
                KeyHint { key: "a-z", desc: "set mark" },
            ],
            '\'' => return vec![
                KeyHint { key: "a-z", desc: "jump mark" },
            ],
            '[' => return vec![
                KeyHint { key: "[", desc: "prev group" },
            ],
            ']' => return vec![
                KeyHint { key: "]", desc: "next group" },
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
        KeyHint { key: "h", desc: "left" },
        KeyHint { key: "l", desc: "right" },
        KeyHint { key: "w", desc: "next word" },
        KeyHint { key: "b", desc: "prev word" },
        KeyHint { key: "e", desc: "end word" },
        KeyHint { key: "G", desc: "bottom" },
        KeyHint { key: "0", desc: "start" },
        KeyHint { key: "$", desc: "end" },
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
    }

    #[test]
    fn test_operator_hints() {
        let mut app = mock_app();
        app.pending_operator = Some(Operator::Delete);
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "j")); // Motion
        assert!(hints.iter().any(|h| h.key == "iw")); // Text object
    }

    #[test]
    fn test_change_op_hints() {
        let mut app = mock_app();
        app.pending_operator = Some(Operator::Change);
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "a" && h.desc == "author"));
    }

    #[test]
    fn test_register_hints() {
        let mut app = mock_app();
        app.pending_register_select = true;
        let hints = get_hints(&app);
        assert!(hints.iter().any(|h| h.key == "+"));
    }
}
