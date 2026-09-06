use std::cmp;

use buffer::{Buffer, LinesIter, Line};

pub mod buffer;
pub mod render;
pub struct BufferView {
    width: usize,
    height: usize,

    line_offset: usize,
    col_offset: usize,

    max_line_width: usize, // longest line width in current view
}

impl BufferView {
    pub fn new() -> BufferView {
        BufferView {
            width: 0,
            height: 0,
            line_offset: 0,
            col_offset: 0,
            max_line_width: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn line_offset(&self) -> usize {
        self.line_offset
    }

    pub fn visible_lines<'a>(
        &self,
        buffer: &'a Buffer,
    ) -> impl Iterator<Item = Line<'a>> {
        buffer
            .lines(self.line_offset, self.height)
            .map(|(index, line)| {
                let line = if self.col_offset < line.len() {
                    &line[self.col_offset..]
                } else {
                    &[]
                };

                (index, line)
            })
    }

    pub fn set_size(&mut self, width: usize, height: usize, buffer: &Buffer) {
        self.width = width;
        self.height = height;
        self.set_line_offset(self.line_offset, buffer);
        self.set_col_offset(self.col_offset);
    }

    pub fn set_line_offset(&mut self, offset: usize, buffer: &Buffer) {
        if offset != self.line_offset {
            self.max_line_width =
                max_line_width(buffer.lines(offset, self.height));
        }
        self.line_offset = offset;
    }

    pub fn set_col_offset(&mut self, offset: usize) {
        self.col_offset = offset
    }

    pub fn scroll_lines(&mut self, lines: isize, buffer: &Buffer) {
        self.set_line_offset(
            self.line_offset.saturating_add_signed(lines),
            buffer,
        );
    }

    pub fn scroll_lines_clamp(&mut self, lines: isize, buffer: &Buffer) {
        self.scroll_lines(lines, buffer);
        self.clamp_line_offset(buffer);
    }

    pub fn scroll_cols(&mut self, cols: isize) {
        self.set_col_offset(self.col_offset.saturating_add_signed(cols));
    }

    pub fn scroll_cols_clamp(&mut self, cols: isize) {
        self.scroll_cols(cols);
        self.clamp_col_offset();
    }

    pub fn scroll_to_line_end(&mut self, buffer: &Buffer) {
        self.line_offset = buffer.line_count().saturating_sub(self.height);
    }

    pub fn scroll_to_col_end(&mut self) {
        self.col_offset = self.max_line_width.saturating_sub(self.width);
    }

    fn clamp_line_offset(&mut self, buffer: &Buffer) {
        let visible_max_offset = buffer.line_count().saturating_sub(self.height);
        self.line_offset = cmp::min(self.line_offset, visible_max_offset);
    }

    fn clamp_col_offset(&self) -> usize {
        let visible_max_offset = self.max_line_width.saturating_sub(self.width);
        cmp::min(self.col_offset, visible_max_offset)
    }
}

fn max_line_width(lines: LinesIter<'_>) -> usize {
    lines.map(|(_, line)| line.len()).max().unwrap_or(0)
}

