use std::cmp::min;
use std::io::{self, Stdout};
use std::panic;

use crossterm::terminal::{DisableLineWrap, EnableLineWrap};
use crossterm::{
    QueueableCommand, cursor, execute,
    style::{self, Attribute, Print},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use super::ContentView;

#[derive(Debug)]
pub struct Terminal {
    width: u16,
    height: u16,
}

pub struct RenderConfig {
    pub line_numbers: bool,
}

impl Terminal {
    pub fn init() -> io::Result<Terminal> {
        let (width, height) = terminal::size()?;
        
        initialize_terminal()?;

        Ok(Terminal { width, height })
    }

    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
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
        DisableLineWrap,
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
        EnableLineWrap
    )?;

    Ok(())
}

pub fn clear_screen(stdout: &mut Stdout) -> io::Result<()> {
    stdout.queue(Clear(ClearType::All))?;
    Ok(())
}

pub fn draw_content(
    stdout: &mut Stdout,
    terminal: &Terminal,
    content_view: &ContentView,
    render_config: &RenderConfig,
) -> io::Result<()> {
    let (buffer, row_offset) = (&content_view.buffer, content_view.row_offset);
    let file_rows = content_view.buffer.len();

    stdout.queue(cursor::MoveTo(0, 0))?;
    for (row_number, row) in buffer
        .iter()
        .enumerate()
        .take(min(row_offset + terminal.height() as usize - 1, file_rows))
        .skip(row_offset)
    {
        if render_config.line_numbers {
            stdout.queue(Print(format!("{:>3} ", row_number + 1)))?;
        }

        stdout.queue(Print(row))?.queue(cursor::MoveToNextLine(1))?;
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
    let percentage = ((current_row as f64 / file_rows as f64) * 100.0) as usize;

    stdout
        .queue(cursor::MoveTo(0, terminal.height() - 1))?
        .queue(style::SetAttribute(Attribute::Reverse))?
        .queue(Print(format!("{: <1$}", "", terminal.width() as usize - 1)))?
        .queue(cursor::MoveToColumn(0))?
        .queue(Print("Esc to quit"))?
        .queue(cursor::MoveToColumn(terminal.width() - 15))?
        .queue(Print(format!(
            "{:>3}:{:>3} ({:>3}%)",
            current_row + 1, file_rows, percentage
        )))?
        .queue(style::SetAttribute(Attribute::Reset))?;

    Ok(())
}
