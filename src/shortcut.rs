use global_hotkey::hotkey::{Code, HotKey, Modifiers};

/// Parses a human-readable shortcut string like `"Alt+Q"`, `"Ctrl+Shift+T"`, `"Ctrl+Q"`, `"F8"`
/// into a cross-platform `global_hotkey::hotkey::HotKey`.
pub fn parse_shortcut(spec: &str) -> Option<HotKey> {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("None") {
        return None;
    }

    let mut mods = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in trimmed.split('+').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => mods |= Modifiers::CONTROL,
            "ALT" | "OPTION" | "OPT" => mods |= Modifiers::ALT,
            "SHIFT" => mods |= Modifiers::SHIFT,
            "WIN" | "SUPER" | "CMD" | "COMMAND" | "META" => mods |= Modifiers::SUPER,
            k => {
                key_code = match_code(k);
            }
        }
    }

    let code = key_code?;
    let mods_opt = if mods.is_empty() { None } else { Some(mods) };
    Some(HotKey::new(mods_opt, code))
}

fn match_code(k: &str) -> Option<Code> {
    match k {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" | "DIGIT0" => Some(Code::Digit0),
        "1" | "DIGIT1" => Some(Code::Digit1),
        "2" | "DIGIT2" => Some(Code::Digit2),
        "3" | "DIGIT3" => Some(Code::Digit3),
        "4" | "DIGIT4" => Some(Code::Digit4),
        "5" | "DIGIT5" => Some(Code::Digit5),
        "6" | "DIGIT6" => Some(Code::Digit6),
        "7" | "DIGIT7" => Some(Code::Digit7),
        "8" | "DIGIT8" => Some(Code::Digit8),
        "9" | "DIGIT9" => Some(Code::Digit9),
        "SPACE" => Some(Code::Space),
        "BACKQUOTE" | "`" | "~" => Some(Code::Backquote),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        _ => None,
    }
}
