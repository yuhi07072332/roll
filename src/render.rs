use std::{
    cmp,
    io::{self, Stdout, Write},
};

use crossterm::{
    Command, QueueableCommand, cursor,
    style::{self, Attribute, Print, PrintStyledContent, Stylize},
    terminal::{Clear, ClearType},
};

const FRAME_BUFFER_INIT_CAPACITY: usize = 500;

use crate::buffer::{Buffer, BufferView};

#[derive(Clone, Copy)]
pub struct ScreenSize(pub u16, pub u16);

pub enum SourceName {
    FileName(String),
    Stdin
}

pub struct RenderConfig {
    pub line_numbers: bool,
    pub source_name: SourceName,
}

struct FrameBuf(Vec<u8>);

impl FrameBuf {
    fn new() -> FrameBuf {
        FrameBuf(Vec::with_capacity(FRAME_BUFFER_INIT_CAPACITY))
    }

    fn get_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }

    fn queue(&mut self, buf: &[u8]) -> io::Result<&mut Self> {
        self.0.write_all(buf)?;
        Ok(self)
    }

    fn queue_cmd(&mut self, command: impl Command) -> io::Result<&mut Self> {
        self.0.queue(command)?;
        Ok(self)
    }

    fn flush(&mut self, stdout: &mut Stdout) -> io::Result<()> {
        stdout.write_all(&self.0)?;
        stdout.flush()
    }
}
pub struct Renderer {
    stdout: Stdout,
    config: RenderConfig,

    frame_buf: FrameBuf,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            stdout: io::stdout(),
            config,
            frame_buf: FrameBuf::new(),
        }
    }

    pub fn resize_view(
        &self,
        view: &mut BufferView,
        screen_size: ScreenSize,
        buffer: &Buffer,
    ) {
        let ScreenSize(width, height) = screen_size;
        let width = if self.config.line_numbers {
            (width as usize).saturating_sub(Self::line_number_width(buffer) + 1)
        } else {
            width as usize
        };
        let height = (height as usize).saturating_sub(1);
        view.set_size(width, height, buffer);
    }

    pub fn draw_frame(
        &mut self,
        buffer: &Buffer,
        view: &BufferView,
        size: ScreenSize,
    ) -> io::Result<()> {
        self.frame_buf.get_mut().clear();

        self.clear_screen()?;
        self.draw_view(buffer, view)?;
        self.draw_status_line(view, buffer, size)?;
        self.frame_buf.flush(&mut self.stdout)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.frame_buf.queue_cmd(Clear(ClearType::All)).map(|_| ())
    }

    fn line_number_width(buffer: &Buffer) -> usize {
        (buffer.line_count().checked_ilog10().unwrap_or(0) + 1) as usize
    }

    fn draw_view(
        &mut self,
        buffer: &Buffer,
        view: &BufferView,
    ) -> io::Result<()> {
        self.frame_buf.queue_cmd(cursor::MoveTo(0, 0))?;
        for (row_number, row) in view.visible_lines(buffer) {
            if self.config.line_numbers {
                self.frame_buf.queue_cmd(PrintStyledContent(
                    format!(
                        "{:>width$} ",
                        row_number + 1,
                        width = Self::line_number_width(buffer),
                    )
                    .dim(),
                ))?;
            }

            self.frame_buf
                .queue(&row[..cmp::min(view.width(), row.len())])?
                .queue_cmd(cursor::MoveToNextLine(1))?;
        }

        Ok(())
    }

    fn draw_status_line(
        &mut self,
        view: &BufferView,
        buffer: &Buffer,
        size: ScreenSize,
    ) -> io::Result<()> {
        let ScreenSize(width, height) = size;
        let row_index = view.row_offset();
        let percentage = row_index
            .checked_mul(100)
            .and_then(|n| n.checked_div(buffer.line_count()))
            .unwrap_or(0);
        let source_name = match &self.config.source_name {
            SourceName::FileName(filename) => filename,
            SourceName::Stdin if buffer.is_reading() => "(stdin: reading)",
            SourceName::Stdin => "(stdin)"
        };

        self.frame_buf
            .queue_cmd(cursor::MoveTo(0, height.saturating_sub(1)))?
            .queue_cmd(style::SetAttribute(Attribute::Reverse))?
            .queue_cmd(Print(format!("{: <1$}", "", width as usize)))?;

        self.frame_buf
            .queue_cmd(cursor::MoveToColumn(0))?
            .queue_cmd(Print(source_name))?
            .queue_cmd(cursor::MoveToColumn(width.saturating_sub(15)))?
            .queue_cmd(Print(format!(
                "({:>3}/{:>3}) {:>3}%",
                row_index + 1,
                buffer.line_count(),
                percentage
            )))?
            .queue_cmd(style::SetAttribute(Attribute::Reset))?;

        Ok(())
    }
}
