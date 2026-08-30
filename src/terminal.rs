use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::{io, panic};

#[derive(Debug)]
pub struct Terminal {
    pub width: u16,
    pub height: u16,
}

impl Terminal {
    pub fn init() -> io::Result<Terminal> {
        let (width, height) = terminal::size()?;

        initialize_terminal()?;

        Ok(Terminal { width, height })
    }
}

impl Drop for Terminal {
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
        EnableMouseCapture,
        Clear(ClearType::All),
    )?;

    set_panic_hook();
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}
