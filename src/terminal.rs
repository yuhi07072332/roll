use crossterm::{
    execute,
    terminal::{
        self, Clear, ClearType, DisableLineWrap, EnableLineWrap,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
    event::{ Event },
};
use std::{io, panic};

#[derive(Debug)]
pub struct TerminalGuard {
    width: u16,
    height: u16,
}

impl TerminalGuard {
    pub fn init() -> io::Result<TerminalGuard> {
        let (width, height) = terminal::size()?;

        initialize_terminal()?;

        Ok(TerminalGuard { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn on_event(&mut self, event: Event) {
        if let Event::Resize(width, height) = event {
            self.width = width;
            self.height = height;
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
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
        DisableLineWrap,
        Clear(ClearType::All),
    )?;

    set_panic_hook();
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, EnableLineWrap)?;

    Ok(())
}
