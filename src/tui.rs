use crossterm::{
    QueueableCommand, event::{DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste, EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEvent, poll, read}, execute, terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen}
};

use std::{io::{self, Write}, time::Duration};

#[derive(Debug)]
pub struct Terminal {
    width: u16,
    height: u16,
}

impl Terminal {
    pub fn init() -> io::Result<Terminal> {
        let (width, height) = terminal::size()?;

        terminal::enable_raw_mode()?;

        execute!(
            io::stdout(),
            EnterAlternateScreen,
            Clear(ClearType::All)
        )?;

        Ok(Terminal { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn restore(&self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn print_events() -> io::Result<()> {
    execute!(
         std::io::stdout(),
         EnableBracketedPaste,
         EnableFocusChange,
         EnableMouseCapture
    )?;
    loop {
        // `poll()` waits for an `Event` for a given time period
        if poll(Duration::from_millis(500))? {
            // It's guaranteed that the `read()` won't block when the `poll()`
            // function returns `true`
            match read()? {
                Event::FocusGained => println!("FocusGained\r"),
                Event::FocusLost => println!("FocusLost\r"),
                Event::Key(KeyEvent{code: KeyCode::Esc, ..}) => break,
                Event::Key(event) => println!("{:?}\r", event),
                Event::Mouse(event) => println!("{:?}\r", event),
                Event::Paste(data) => println!("Pasted {:?}\r", data),
                Event::Resize(width, height) => println!("New size {}x{}\r", width, height),
            }
        } else {
            // Timeout expired and no `Event` is available
        }
    }

    execute!(
        std::io::stdout(),
        DisableBracketedPaste,
        DisableFocusChange,
        DisableMouseCapture
    )?;

    Ok(())
}

