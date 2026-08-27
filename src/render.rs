use std::io::{self, Stdout, Write};

use crossterm::{
    QueueableCommand, cursor,
    style::{self, Attribute, Print},
    terminal::{Clear, ClearType},
};

use super::BufferView;
use crate::terminal::TerminalGuard;

pub struct RenderConfig {
    pub line_numbers: bool,
}

pub struct Renderer {
    stdout: Stdout,
    config: RenderConfig,
    width: u16,
    height: u16
}

impl Renderer {
    pub fn new(config: RenderConfig, terminal: &TerminalGuard) -> Self {
        Self {
            stdout: io::stdout(),
            config,
            width: terminal.width(),
            height: terminal.height()
        }
    }

    pub fn draw_frame(
        &mut self,
        view: &BufferView,
        terminal: &TerminalGuard,
    ) -> io::Result<()> {
        let (width, height) = terminal.size();
        self.width = width;
        self.height = height;

        clear_screen(&mut self.stdout)?;
        self.draw_view(view)?;
        self.draw_status_line(view)?;
        self.stdout.flush()
    }

    fn draw_view(&mut self, view: &BufferView) -> io::Result<()> {
        self.stdout.queue(cursor::MoveTo(0, 0))?;
        for (row_number, row) in view
            .lines_from_offset(self.height as usize - 1)
        {
            if self.config.line_numbers {
                self.stdout
                    .queue(Print(format!("{:>3} ", row_number + 1)))?;
            }

            self.stdout.write_all(row)?;
            self.stdout.queue(cursor::MoveToNextLine(1))?;
        }

        Ok(())
    }

    fn draw_status_line(&mut self, view: &BufferView) -> io::Result<()> {
        let row_index = view.row_offset();
        let percentage =
            ((row_index as f64 / view.len() as f64) * 100.0) as usize;

        self.stdout
            .queue(cursor::MoveTo(0, self.height - 1))?
            .queue(style::SetAttribute(Attribute::Reverse))?
            .queue(Print(format!("{: <1$}", "", self.width as usize - 1)))?;

        self.stdout
            .queue(cursor::MoveToColumn(0))?
            .queue(Print("Esc to quit"))?
            .queue(cursor::MoveToColumn(self.width - 15))?
            .queue(Print(format!(
                "{:>3}:{:>3} ({:>3}%)",
                row_index + 1,
                view.len(),
                percentage
            )))?
            .queue(style::SetAttribute(Attribute::Reset))?;

        Ok(())
    }
}

fn clear_screen(stdout: &mut Stdout) -> io::Result<()> {
    stdout.queue(Clear(ClearType::All))?;
    Ok(())
}
