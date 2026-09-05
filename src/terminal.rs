use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    style::ResetColor,
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::{io, panic};

#[derive(Clone, Copy)]
pub struct ScreenSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug)]
pub struct TerminalGuard {}

impl TerminalGuard {
    pub fn init() -> io::Result<TerminalGuard> {
        initialize_terminal()?;

        Ok(TerminalGuard {})
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

pub fn size() -> io::Result<ScreenSize> {
    let (width, height) = terminal::size()?;
    Ok(ScreenSize { width, height })
}

fn set_panic_hook() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        default_hook(panic_info);
    }));
}

fn initialize_terminal() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        Clear(ClearType::All),
    )?;

    set_panic_hook();
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        ResetColor,
        cursor::Show
    )?;

    Ok(())
}
