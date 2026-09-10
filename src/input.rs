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
            code => {
                let Some(sp) = SpecialKey::new(code) else {
                    return Key::Unknown;
                };

                match modifiers {
                    KeyModifiers::NONE => Key::Sp(sp),
                    KeyModifiers::CONTROL => Key::Ctrl(sp),
                    KeyModifiers::SHIFT => Key::Shift(sp),
                    _ => Key::Unknown,
                }
            },
        }
    }
}

impl SpecialKey {
    fn new(code: KeyCode) -> Option<SpecialKey> {
        match code {
            KeyCode::Esc => Some(SpecialKey::Esc),
            KeyCode::Enter => Some(SpecialKey::Enter),
            KeyCode::Backspace => Some(SpecialKey::Backspace),
            KeyCode::Up => Some(SpecialKey::Up),
            KeyCode::Down => Some(SpecialKey::Down),
            KeyCode::Left => Some(SpecialKey::Left),
            KeyCode::Right => Some(SpecialKey::Right),
            KeyCode::PageUp => Some(SpecialKey::PageUp),
            KeyCode::PageDown => Some(SpecialKey::PageDown),
            KeyCode::Home => Some(SpecialKey::Home),
            KeyCode::End => Some(SpecialKey::End),
            _ => None
        }
    }
}
