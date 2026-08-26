use crossterm::{
    QueueableCommand, cursor, execute,
    style::{self, Attribute, Print},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::io::{self, Stdout, Write};
use std::cmp::{min, max};

use super::ContentView;

#[derive(Debug)]
pub struct Terminal {
    width: u16,
    height: u16,
}

impl Terminal {
    pub fn init() -> io::Result<Terminal> {
        let (width, height) = terminal::size()?;

        terminal::enable_raw_mode()?;

        execute!(io::stdout(), EnterAlternateScreen, Clear(ClearType::All))?;

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

pub fn draw_content(
    stdout: &mut Stdout,
    terminal: &Terminal,
    content_view: &ContentView,
) -> io::Result<()> {
    let (buffer, row_offset) = (&content_view.buffer, content_view.row_offset);
    let file_rows = content_view.buffer.len();

    stdout.queue(cursor::MoveTo(0, 0))?;
    for row in 0..min(terminal.height() as usize, file_rows) {
        let file_row: usize = row + row_offset;
        stdout
            .queue(Print(&buffer[file_row]))?
            .queue(cursor::MoveToNextLine(1))?;
    }

    Ok(())
}

pub fn draw_status_line(
    stdout: &mut Stdout,
    terminal: &Terminal,
    content_view: &ContentView,
) -> io::Result<()> {
    let file_rows = content_view.buffer.len();
    let current_row = content_view.row_offset;

    stdout
        .queue(cursor::MoveTo(0, terminal.height() - 1))?
        .queue(style::SetAttribute(Attribute::Reverse))?
        .queue(Print("Esc to quit"))?
        .queue(cursor::MoveToColumn(terminal.width() - 8))?
        .queue(Print(format!("{:>3}:{:>3}", current_row, file_rows)))?;

    Ok(())
}
