use std::{
    cmp,
    fs::{self},
    io,
    path::Path,
};

pub const BYTES_PER_READ: usize = 512;

#[derive(Default)]
pub struct Buffer {
    data: Vec<u8>,
    line_starts: Vec<usize>,
    is_reading: bool,
}

impl Buffer {
    pub fn from_file(path: &Path) -> io::Result<Buffer> {
        let data = fs::read(path)?;
        let mut line_starts: Vec<usize> = Vec::new();
        scan_line_starts(&mut line_starts, &data, 0);

        Ok(Buffer {
            data,
            line_starts,
            is_reading: false,
        })
    }

    pub fn from_stdin() -> io::Result<Buffer> {
        Ok(Buffer {
            data: Vec::new(),
            line_starts: Vec::new(),
            is_reading: true,
        })
    }

    pub fn is_reading(&self) -> bool {
        self.is_reading
    }

    pub fn line_at(&self, index: usize) -> Option<&[u8]> {
        let begin_idx = *self.line_starts.get(index)?;
        let end_idx = *self.line_starts.get(index + 1)?;

        let mut line = &self.data[begin_idx..end_idx];
        if let Some(last) = line.last()
            && *last == b'\n'
        {
            line = &line[..line.len() - 1];

            if let Some(last) = line.last()
                && *last == b'\r'
            {
                line = &line[..line.len() - 1];
            }
        }

        Some(line)
    }

    pub fn on_buffer_read(&mut self, n: usize, bytes: Box<[u8]>) {
        self.append_bytes(&bytes.as_ref()[..n]);
    }

    pub fn on_buffer_eof(&mut self) {
        self.is_reading = false
    }

    pub fn lines<'a>(&'a self, from: usize, take: usize) -> LinesIter<'a> {
        LinesIter {
            buf: self,
            current_index: from,
            last_index: from + take,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len().saturating_sub(1)
    }

    fn append_bytes(&mut self, bytes: &[u8]) {
        // Remove sentinel if has
        if let Some(c) = self.data.last()
            && *c != b'\n'
        {
            self.line_starts.pop();
        }

        let old_len = self.data.len();
        self.data.extend_from_slice(bytes);
        scan_line_starts(&mut self.line_starts, &self.data, old_len);
    }
}

fn scan_line_starts(out: &mut Vec<usize>, data: &[u8], from_nbyte: usize) {
    if out.is_empty() {
        out.push(from_nbyte)
    }

    out.extend(
        data.iter()
            .enumerate()
            .skip(from_nbyte)
            .filter_map(|(idx, c)| (*c == b'\n').then_some(idx + 1)),
    );

    // Add sentinel for the last line
    if let Some(c) = data.last()
        && *c != b'\n'
    {
        out.push(data.len());
    }
}

/// similar to `BufRead::lines()`
pub struct LinesIter<'a> {
    buf: &'a Buffer,
    current_index: usize,
    last_index: usize,
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.last_index {
            return None;
        }

        let line = (self.current_index, self.buf.line_at(self.current_index)?);
        self.current_index += 1;
        Some(line)
    }
}

pub struct BufferView {
    width: usize,
    height: usize,

    row_offset: usize,
    col_offset: usize,

    max_line_width: usize, // longest line width in current view
}

impl BufferView {
    pub fn new() -> BufferView {
        BufferView {
            width: 0,
            height: 0,
            row_offset: 0,
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

    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn visible_lines<'a>(
        &self,
        buffer: &'a Buffer,
    ) -> impl Iterator<Item = (usize, &'a [u8])> {
        buffer
            .lines(self.row_offset, self.height)
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
        self.set_row_offset(self.row_offset, buffer);
        self.set_col_offset(self.col_offset);
    }

    pub fn set_row_offset(&mut self, offset: usize, buffer: &Buffer) {
        let offset = self.clamp_row_offset(offset, buffer);
        if offset != self.row_offset {
            self.max_line_width =
                max_line_width(buffer.lines(offset, self.height));
        }
        self.row_offset = offset;
    }

    pub fn set_col_offset(&mut self, offset: usize) {
        self.col_offset = self.clamp_col_offset(offset);
    }

    pub fn scroll_down(&mut self, lines: usize, buffer: &Buffer) {
        self.set_row_offset(self.row_offset + lines, buffer);
    }

    pub fn scroll_up(&mut self, lines: usize, buffer: &Buffer) {
        self.set_row_offset(self.row_offset.saturating_sub(lines), buffer);
    }

    pub fn scroll_right(&mut self, cols: usize) {
        self.set_col_offset(self.col_offset + cols);
    }

    pub fn scroll_left(&mut self, cols: usize) {
        self.set_col_offset(self.col_offset.saturating_sub(cols));
    }

    pub fn scroll_to_row_end(&mut self, buffer: &Buffer) {
        self.row_offset = buffer.line_count().saturating_sub(self.height);
    }

    pub fn scroll_to_col_end(&mut self) {
        self.col_offset = self.max_line_width.saturating_sub(self.width);
    }

    fn clamp_row_offset(&self, offset: usize, buffer: &Buffer) -> usize {
        let max_offset = buffer.line_count().saturating_sub(self.height);
        cmp::min(offset, max_offset)
    }

    fn clamp_col_offset(&self, offset: usize) -> usize {
        let max_offset = self.max_line_width.saturating_sub(self.width);
        cmp::min(offset, max_offset)
    }
}

fn max_line_width(lines: LinesIter<'_>) -> usize {
    lines.map(|(_, line)| line.len()).max().unwrap_or(0)
}
