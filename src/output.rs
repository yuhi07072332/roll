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

use crate::{
    InputBox, Mode,
    terminal::ScreenSize,
    view::{Buffer, View},
};

pub enum SourceName {
    FileName(String),
    Stdin,
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
    is_cursor_visible: bool,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            stdout: io::stdout(),
            config,
            frame_buf: FrameBuf::new(),
            is_cursor_visible: false,
        }
    }

    pub fn resize_view(
        &self,
        view: &mut View,
        screen_size: ScreenSize,
        buffer: &Buffer,
    ) {
        let ScreenSize { width, height } = screen_size;
        let width = if self.config.line_numbers {
            (width as usize).saturating_sub(line_number_width(buffer) + 1)
        } else {
            width as usize
        };
        let height = (height as usize).saturating_sub(1);
        view.set_size(width, height, buffer);
    }

    pub fn draw_frame(
        &mut self,
        buffer: &Buffer,
        view: &View,
        mode: &Mode,
        size: ScreenSize,
    ) -> io::Result<()> {
        self.frame_buf.get_mut().clear();

        self.clear_screen()?;
        draw_view(&mut self.frame_buf, buffer, view, self.config.line_numbers)?;

        match mode {
            Mode::Normal => {
                let source_name = match &self.config.source_name {
                    SourceName::FileName(filename) => filename,
                    SourceName::Stdin if buffer.is_reading() => {
                        "(stdin: reading)"
                    }
                    SourceName::Stdin => "(stdin)",
                };
                draw_status_line(
                    &mut self.frame_buf,
                    source_name,
                    view,
                    buffer,
                    size,
                )?;
                self.hide_cursor()?
            }
            Mode::Input(input_box) => {
                draw_input_box(&mut self.frame_buf, input_box, size)?;
                self.show_cursor()?
            }
            Mode::Search(state) => {
                let message = format!("search: \'{}\'", state.pattern());
                draw_status_line(
                    &mut self.frame_buf,
                    &message,
                    view,
                    buffer,
                    size,
                )?;
                self.hide_cursor()?
            }
        }

        self.frame_buf.flush(&mut self.stdout)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.frame_buf.queue_cmd(Clear(ClearType::All)).map(|_| ())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        if !self.is_cursor_visible {
            self.frame_buf.queue_cmd(cursor::Show)?;
            self.is_cursor_visible = true;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        if self.is_cursor_visible {
            self.frame_buf.queue_cmd(cursor::Hide)?;
            self.is_cursor_visible = false;
        }
        Ok(())
    }
}

fn line_number_width(buffer: &Buffer) -> usize {
    (buffer.line_count().checked_ilog10().unwrap_or(0) + 1) as usize
}

fn truncate_left(text: &str, width: usize) -> &str {
    &text[cmp::min(
        text.len().saturating_sub(width),
        text.len().saturating_sub(1),
    )..]
}

fn truncate_right(text: &str, width: usize) -> &str {
    &text[..cmp::min(width, text.len().saturating_sub(1))]
}

fn draw_view(
    frame_buf: &mut FrameBuf,
    buffer: &Buffer,
    view: &View,
    show_line_numbers: bool,
) -> io::Result<()> {
    frame_buf.queue_cmd(cursor::MoveTo(0, 0))?;
    for rline in view.visible_lines() {
        if show_line_numbers {
            frame_buf.queue_cmd(PrintStyledContent(
                format!(
                    "{:>width$} ",
                    rline.line_number + 1,
                    width = line_number_width(buffer),
                )
                .dim(),
            ))?;
        }

        // TODO: 
        frame_buf
            .queue(&rline.data.as_bytes())?
            .queue_cmd(cursor::MoveToNextLine(1))?;
    }

    Ok(())
}

fn draw_input_box(
    frame_buf: &mut FrameBuf,
    input_box: &InputBox,
    size: ScreenSize,
) -> io::Result<()> {
    let input: &str = &input_box.input;
    let prefix = input_box.prefix.as_deref().unwrap_or("");
    let input_text = truncate_left(input, size.width as usize - 1);

    frame_buf
        .queue_cmd(cursor::MoveTo(0, size.height.saturating_sub(1)))?
        .queue_cmd(PrintStyledContent(prefix.cyan()))?
        .queue(input_text.as_bytes())?;

    Ok(())
}

fn draw_status_line(
    frame_buf: &mut FrameBuf,
    message: &str,
    view: &View,
    buffer: &Buffer,
    size: ScreenSize,
) -> io::Result<()> {
    let ScreenSize { width, height } = size;
    let row_index =
        cmp::min(view.line_offset() + view.height(), buffer.line_count());
    let percentage = row_index
        .checked_mul(100)
        .and_then(|n| n.checked_div(buffer.line_count()))
        .unwrap_or(0);

    frame_buf
        .queue_cmd(cursor::MoveTo(0, height.saturating_sub(1)))?
        .queue_cmd(style::SetAttribute(Attribute::Reverse))?
        .queue_cmd(Print(format!("{: <1$}", "", width as usize)))?;

    frame_buf
        .queue_cmd(cursor::MoveToColumn(0))?
        .queue_cmd(Print(truncate_right(message, size.width as usize - 15)))?
        .queue_cmd(cursor::MoveToColumn(width.saturating_sub(15)))?
        .queue_cmd(Print(format!(
            "({:>3}/{:>3}) {:>3}%",
            row_index,
            buffer.line_count(),
            percentage
        )))?
        .queue_cmd(style::SetAttribute(Attribute::Reset))?;

    Ok(())
}
