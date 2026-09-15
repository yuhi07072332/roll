use std::{
    cmp,
    io::{self, Stdout, Write},
};

use crossterm::{
    Command, QueueableCommand, cursor,
    style::{self, Attribute, Print, PrintStyledContent, Stylize},
    terminal::{Clear, ClearType},
};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const FRAME_BUFFER_INIT_CAPACITY: usize = 1024;

use crate::{
    InputBox, Mode, Pager,
    terminal::ScreenSize,
    buffer::Buffer,
    view::{RenderedLine, render::raw_index},
};

pub enum SourceName {
    FileName(String),
    Stdin,
}

pub enum LineWrap {
    Disabled,
    HardWrap,
    SoftWrap,
}

pub struct RenderConfig {
    pub source_name: SourceName,
    pub line_numbers: bool,
    pub wrap: LineWrap,
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

    pub fn resize_view(&self, pager: &mut Pager) {
        let ScreenSize { width, height } = pager.screen_size;
        let width = if self.config.line_numbers {
            (width as usize)
                .saturating_sub(line_number_width(&pager.buffer) as usize + 1)
        } else {
            width as usize
        };
        let height = (height as usize).saturating_sub(1);
        pager.view.set_size(width, height, &pager.buffer);
    }

    pub fn draw_frame(&mut self, pager: &Pager, mode: &Mode) -> io::Result<()> {
        self.frame_buf.get_mut().clear();

        self.clear_screen()?;
        draw_view(&mut self.frame_buf, pager, &self.config)?;

        match mode {
            Mode::Normal if let Some(state) = pager.search_state.as_ref() => {
                let message = format!("search: \'{}\'", state.pattern());
                draw_status_line(&mut self.frame_buf, &message, pager)?;
                self.hide_cursor()?
            }
            Mode::Normal => {
                let source_name = match &self.config.source_name {
                    SourceName::FileName(filename) => filename,
                    SourceName::Stdin if pager.buffer.is_reading() => {
                        "(stdin: reading)"
                    }
                    SourceName::Stdin => "(stdin)",
                };
                draw_status_line(&mut self.frame_buf, source_name, pager)?;
                self.hide_cursor()?
            }
            Mode::Input(input_box) => {
                draw_input_box(
                    &mut self.frame_buf,
                    input_box,
                    pager.screen_size,
                )?;
                self.show_cursor()?
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

fn line_number_width(buffer: &Buffer) -> u16 {
    (buffer.line_count().checked_ilog10().unwrap_or(0) + 1) as u16
}

fn truncate_left(text: &str, width: usize) -> &str {
    //TODO: use chars instead of bytes
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
    pager: &Pager,
    config: &RenderConfig,
) -> io::Result<()> {
    frame_buf.queue_cmd(cursor::MoveTo(0, 0))?;
    let mut y = 0;
    let line_number_width = line_number_width(&pager.buffer);

    for rline in pager.view.visible_lines() {
        if y as usize > pager.view.height() {
            break;
        }

        if config.line_numbers {
            frame_buf.queue_cmd(PrintStyledContent(
                format!(
                    "{:>width$} ",
                    rline.line_number + 1,
                    width = line_number_width as usize
                )
                .dim(),
            ))?;
        }

        y += draw_line(frame_buf, rline, pager, config, line_number_width + 1)?;
    }

    while (y as usize) < pager.view.height() {
        frame_buf
            .queue_cmd(PrintStyledContent("~".dim()))?
            .queue_cmd(cursor::MoveToNextLine(1))?;
        y += 1;
    }

    Ok(())
}

fn draw_line(
    frame_buf: &mut FrameBuf,
    rline: &RenderedLine,
    pager: &Pager,
    config: &RenderConfig,
    line_begin_col: u16,
) -> io::Result<u32> {
    let Some(raw) = pager.buffer.line_at(rline.line_number) else {
        return Ok(0);
    };
    let highlights = if let Some(search_state) = &pager.search_state {
        search_state
            .match_at(rline.line_number)
            .map(|v| v.as_slice())
            .unwrap_or_default()
    } else {
        &[]
    };

    let mut hl_index = 0;
    let mut line_height: u32 = 1;
    let mut current_line_width = 0;
    let mut is_highlighting = false;
    for (rx, gr) in rline.data.grapheme_indices(true) {
        // TODO: handle control characters(escape sequence, etc.)
        let gr_width = gr.width();
        current_line_width += gr_width;
        if current_line_width > pager.view.width() {
            match config.wrap {
                LineWrap::Disabled => break,
                LineWrap::HardWrap => {
                    frame_buf
                        .queue_cmd(cursor::MoveToNextLine(1))?
                        .queue_cmd(cursor::MoveToColumn(line_begin_col))?;
                    line_height += 1;
                    current_line_width = gr_width;
                }
                // TODO:
                LineWrap::SoftWrap => (),
            }
        }

        let raw_index = raw_index(rx, raw, &pager.rl_config);
        if let Some(hl) = highlights.get(hl_index) {
            if hl.start == raw_index {
                frame_buf.queue(b"\x1b[7m")?;
                is_highlighting = true;
            } else if raw_index == hl.end {
                frame_buf.queue(b"\x1b[m")?;
                is_highlighting = false;
                hl_index += 1;
            }
        }

        frame_buf.queue(gr.as_bytes())?;
    }

    if is_highlighting {
        frame_buf.queue(b"\x1b[m")?;
    }
    frame_buf.queue_cmd(cursor::MoveToNextLine(1))?;

    Ok(line_height)
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
    pager: &Pager,
) -> io::Result<()> {
    let Pager { view, buffer, .. } = pager;
    let ScreenSize { width, height } = pager.screen_size;
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
        .queue_cmd(Print(truncate_right(message, width as usize - 15)))?
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
