use std::{
    cmp,
    fs::{self},
    io,
    ops::Range,
    path::Path,
};

pub type Line<'a> = (usize, &'a [u8]);

pub const BYTES_PER_READ: usize = 512;

#[derive(Debug)]
pub struct Buffer {
    data: Vec<u8>,
    line_starts: Vec<usize>,
    is_reading: bool,
}

impl Buffer {
    pub fn from_file(path: &Path) -> io::Result<Buffer> {
        Ok(Self::from_bytes(fs::read(path)?))
    }

    pub fn from_stdin() -> Buffer {
        Self::default()
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
            end_index: from + take,
        }
    }

    pub fn lines_in<'a>(&'a self, range: Range<usize>) -> LinesIter<'a> {
        LinesIter {
            buf: self,
            current_index: range.start,
            end_index: range.end,
        }
    }

    #[allow(dead_code)]
    pub fn lines_from<'a>(&'a self, from: usize) -> LinesIter<'a> {
        LinesIter {
            buf: self,
            current_index: from,
            end_index: self.line_count()
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len().saturating_sub(1)
    }

    fn from_bytes(data: Vec<u8>) -> Buffer {
        let mut line_starts: Vec<usize> = Vec::new();
        scan_line_starts(&mut line_starts, &data, 0);

        Buffer {
            data,
            line_starts,
            is_reading: false,
        }
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

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer {
            data: Vec::new(),
            line_starts: Vec::new(),
            is_reading: true,
        }
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
    end_index: usize,
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.end_index {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(bytes: &[u8]) -> Buffer {
        Buffer::from_bytes(Vec::from(bytes))
    }

    #[test]
    fn buffer_lines_iter() {
        let buf = buffer(b"first line\nsecond line\nthird line");
        let mut iter = buf.lines_from(0);

        assert!(!buf.is_reading());
        assert_eq!(buf.line_count(), 3);
        assert_eq!(iter.next(), Some((0usize, &b"first line"[..])));
        assert_eq!(iter.next(), Some((1usize, &b"second line"[..])));
        assert_eq!(iter.next(), Some((2usize, &b"third line"[..])));
    }

    #[test]
    fn buffer_lines_handles_crnl() {
        let buf = buffer(b"first line\r\nsecond\rline\r\nthird line\n\r");

        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.line_at(0), Some(&b"first line"[..]));
        assert_eq!(buf.line_at(1), Some(&b"second\rline"[..]));
        assert_eq!(buf.line_at(2), Some(&b"third line"[..]));
        assert_eq!(buf.line_at(3), Some(&b"\r"[..]));
    }


    #[test]
    fn line_count_handles_empty_and_trailing_newline_input() {
        let buf = buffer(b"a\n\nb\nc");

        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.line_at(0), Some(&b"a"[..]));
        assert_eq!(buf.line_at(1), Some(&b""[..]));
        assert_eq!(buf.line_at(2), Some(&b"b"[..]));
        assert_eq!(buf.line_at(3), Some(&b"c"[..]));
    }

    #[test]
    fn buffer_accumulates_bytes() {
        let mut buf = Buffer::default();

        buf.append_bytes(b"hello");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some(&b"hello"[..]));

        buf.append_bytes(b" world\n");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_at(0), Some(&b"hello world"[..]));

        buf.append_bytes(b"!!!");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.line_at(0), Some(&b"hello world"[..]));
        assert_eq!(buf.line_at(1), Some(&b"!!!"[..]));

        buf.append_bytes(b"\nabcde\nfg");
        assert_eq!(buf.line_at(1), Some(&b"!!!"[..]));
        assert_eq!(buf.line_at(2), Some(&b"abcde"[..]));
        assert_eq!(buf.line_at(3), Some(&b"fg"[..]));
    }
}
