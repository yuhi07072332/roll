use std::io::{self, Stdout, Write};

use crossterm::{
    QueueableCommand, cursor,
    style::{self, Attribute, Print},
    terminal::{Clear, ClearType},
};

use super::BufferView;

#[derive(Clone, Copy)]
pub struct ScreenSize(pub u16, pub u16);

pub struct RenderConfig {
    pub line_numbers: bool,
}

pub struct Renderer {
    stdout: Stdout,
    config: RenderConfig,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            stdout: io::stdout(),
            config,
        }
    }

    pub fn draw_frame(
        &mut self,
        view: &BufferView,
        size: ScreenSize
    ) -> io::Result<()> {
        clear_screen(&mut self.stdout)?;
        self.draw_view(view, size)?;
        self.draw_status_line(view, size)?;
        self.stdout.flush()
    }

    fn draw_view(&mut self, view: &BufferView, size: ScreenSize) -> io::Result<()> {
        let ScreenSize(_, height) = size;

        self.stdout.queue(cursor::MoveTo(0, 0))?;
        for (row_number, row) in
            view.visible_lines(height.saturating_sub(1u16) as usize)
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

    fn draw_status_line(&mut self, view: &BufferView, size: ScreenSize) -> io::Result<()> {
        let ScreenSize(width, height) = size;
        let row_index = view.row_offset();
        let percentage = row_index
            .checked_div(view.line_count())
            .unwrap_or(0)
            * 100;

        self.stdout
            .queue(cursor::MoveTo(0, height.saturating_sub(1)))?
            .queue(style::SetAttribute(Attribute::Reverse))?
            .queue(Print(format!("{: <1$}", "", width as usize - 1)))?;

        self.stdout
            .queue(cursor::MoveToColumn(0))?
            .queue(Print("Esc to quit"))?
            .queue(cursor::MoveToColumn(width.saturating_sub(15)))?
            .queue(Print(format!(
                "{:>3}:{:>3} ({:>3}%)",
                row_index + 1,
                view.line_count(),
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
