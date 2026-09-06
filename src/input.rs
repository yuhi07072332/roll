use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy)]
pub enum Key {
    Char(char),
    CtrlChar(char),

    Sp(SpecialKey),

    #[allow(dead_code)]
    Ctrl(SpecialKey),
    #[allow(dead_code)]
    Shift(SpecialKey),

    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum SpecialKey {
    Esc,
    Enter,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    PageDown,
    PageUp,
    Home,
    End,
}

impl Key {
    pub fn new(key_event: KeyEvent) -> Key {
        let KeyEvent {
            code, modifiers, ..
        } = key_event;

        match code {
            KeyCode::Char(c) => match modifiers {
                KeyModifiers::NONE | KeyModifiers::SHIFT => Key::Char(c),
                KeyModifiers::CONTROL => Key::CtrlChar(c),
                _ => Key::Unknown,
            },
            code => match modifiers {
                KeyModifiers::NONE => Key::Sp(SpecialKey::new(code)),
                KeyModifiers::CONTROL => Key::Ctrl(SpecialKey::new(code)),
                KeyModifiers::SHIFT => Key::Shift(SpecialKey::new(code)),
                _ => Key::Unknown,
            },
        }
    }
}

impl SpecialKey {
    fn new(code: KeyCode) -> SpecialKey {
        match code {
            KeyCode::Esc => SpecialKey::Esc,
            KeyCode::Enter => SpecialKey::Enter,
            KeyCode::Backspace => SpecialKey::Backspace,
            KeyCode::Up => SpecialKey::Up,
            KeyCode::Down => SpecialKey::Down,
            KeyCode::Left => SpecialKey::Left,
            KeyCode::Right => SpecialKey::Right,
            KeyCode::PageUp => SpecialKey::PageUp,
            KeyCode::PageDown => SpecialKey::PageDown,
            KeyCode::Home => SpecialKey::Home,
            KeyCode::End => SpecialKey::End,
            _ => panic!("SpecialKey::new()"),
        }
    }
}
