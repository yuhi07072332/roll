use std::{
    cmp,
    io::{self, Stdout, Write},
};

use crossterm::{
    Command, QueueableCommand, cursor, style::{self, Attribute, Print, PrintStyledContent, Stylize}, terminal::{Clear, ClearType}
};

const FRAME_BUFFER_INIT_CAPACITY: usize = 500;

use super::BufferView;

#[derive(Clone, Copy)]
pub struct ScreenSize(pub u16, pub u16);

pub struct RenderConfig {
    pub line_numbers: bool,
}

pub struct Renderer {
    stdout: Stdout,
    config: RenderConfig,

    frame_buf: Vec<u8>
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            stdout: io::stdout(),
            config,
            frame_buf: Vec::with_capacity(FRAME_BUFFER_INIT_CAPACITY)
        }
    }

    pub fn resize_view(&self, view: &mut BufferView, screen_size: ScreenSize) {
        let ScreenSize(width, height) = screen_size;
        let width = if self.config.line_numbers {
            (width as usize).saturating_sub(Self::line_number_width(view) + 1)
        } else {
            width as usize
        };
        let height = (height as usize).saturating_sub(1);
        view.set_size(width, height);
    }

    pub fn draw_frame(
        &mut self,
        view: &BufferView,
        size: ScreenSize,
    ) -> io::Result<()> {
        self.frame_buf.clear();

        self.clear_screen()?;
        self.draw_view(view)?;
        self.draw_status_line(view, size)?;
        self.flush()
    }

    fn queue(&mut self, buf: &[u8]) -> io::Result<&mut Self> {
        self.frame_buf.write_all(buf)?;
        Ok(self)
    }

    fn queue_cmd(&mut self, command: impl Command) -> io::Result<&mut Self> {
        self.frame_buf.queue(command)?;
        Ok(self)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.write_all(&self.frame_buf)?;
        self.stdout.flush()
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.queue_cmd(Clear(ClearType::All)).map(|_| ())
    }

    fn line_number_width(view: &BufferView) -> usize {
        (view.line_count().checked_ilog10().unwrap_or(0) + 1) as usize
    }

    fn draw_view(&mut self, view: &BufferView) -> io::Result<()> {
        self.queue_cmd(cursor::MoveTo(0, 0))?;
        for (row_number, row) in view.visible_lines() {
            if self.config.line_numbers {
                self.queue_cmd(PrintStyledContent(
                    format!(
                        "{:>width$} ",
                        row_number + 1,
                        width = Self::line_number_width(view),
                    )
                    .dim(),
                ))?;
            }

            self.queue(&row[..cmp::min(view.width(), row.len())])?;
            self.queue_cmd(cursor::MoveToNextLine(1))?;
        }

        Ok(())
    }

    fn draw_status_line(
        &mut self,
        view: &BufferView,
        size: ScreenSize,
    ) -> io::Result<()> {
        let ScreenSize(width, height) = size;
        let row_index = view.row_offset();
        let percentage = row_index
            .checked_mul(100)
            .and_then(|n| n.checked_div(view.line_count()))
            .unwrap_or(0);

        self.queue_cmd(cursor::MoveTo(0, height.saturating_sub(1)))?
            .queue_cmd(style::SetAttribute(Attribute::Reverse))?
            .queue_cmd(Print(format!(
                "{: <1$}",
                "",
                (width as usize).saturating_sub(1)
            )))?;

        self.queue_cmd(cursor::MoveToColumn(0))?
            .queue_cmd(Print("Esc to quit"))?
            .queue_cmd(cursor::MoveToColumn(width.saturating_sub(15)))?
            .queue_cmd(Print(format!(
                "{:>3}:{:>3} ({:>3}%)",
                row_index + 1,
                view.line_count(),
                percentage
            )))?
            .queue_cmd(style::SetAttribute(Attribute::Reset))?;

        Ok(())
    }
}
