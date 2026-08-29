use std::{
    cmp,
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

#[derive(Default)]
pub struct Buffer {
    data: Vec<u8>,
    line_idx: Vec<(usize, usize)>,
    max_line_len: usize,

    last_line_begin_idx: usize,
    is_reading: bool,
}

impl Buffer {
    pub fn from_file(path: &Path) -> io::Result<Buffer> {
        let mut buf = Buffer::default();
        let file = File::open(path)?;
        buf.append_bytes(BufReader::new(file).bytes())?;
        Ok(buf)
    }

    pub fn from_stdin() -> io::Result<Buffer> {
        // TODO: spawn a new thread to read stdin instead of blocking the main thread
        let mut buf = Buffer::default();
        buf.append_bytes(BufReader::new(io::stdin()).bytes())?;
        Ok(buf)
    }

    pub fn line_at(&self, index: usize) -> Option<&[u8]> {
        let (begin_idx, end_idx) = *self.line_idx.get(index)?;
        Some(&self.data[begin_idx..end_idx])
    }

    pub fn lines_from(&self, from: usize) -> impl Iterator<Item = &[u8]> {
        LinesIter {
            buf: self,
            current_line: from,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_idx.len()
    }

    fn append_bytes(
        &mut self,
        bytes: impl Iterator<Item = io::Result<u8>>,
    ) -> io::Result<()> {
        let mut append_line = |nl_idx: usize, after_cr: bool| {
            let line_end_idx = if after_cr { nl_idx - 1 } else { nl_idx };
            self.line_idx.push((self.last_line_begin_idx, line_end_idx));
            self.max_line_len = cmp::max(
                line_end_idx - self.last_line_begin_idx,
                self.max_line_len,
            );
            self.last_line_begin_idx = nl_idx + 1;
        };

        let mut after_cr = false;

        for byte in bytes {
            let byte = byte?;
            self.data.push(byte);
            let byte_idx = self.data.len() - 1;

            match byte {
                b'\r' => after_cr = true,
                b'\n' => {
                    append_line(
                        if after_cr { byte_idx - 1 } else { byte_idx },
                        after_cr,
                    );
                    after_cr = false;
                }
                _ => after_cr = false,
            }
        }

        // reached EOF
        self.is_reading = false;
        if self.data.last().filter(|byte| **byte != b'\n').is_some() {
            append_line(self.data.len() - 1, false);
        }

        Ok(())
    }
}

/// similar to `BufRead::lines()`
struct LinesIter<'a> {
    buf: &'a Buffer,
    current_line: usize,
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.buf.line_at(self.current_line)?;
        self.current_line += 1;
        Some(line)
    }
}

/// A rectangle view of a existing Buffer.
pub struct BufferView<'a> {
    buffer: &'a Buffer,
    width: usize,
    height: usize,

    row_offset: usize,
    col_offset: usize, // TODO:
}

impl<'a> BufferView<'a> {
    pub fn new(
        buffer: &'a Buffer,
        width: usize,
        height: usize,
    ) -> BufferView<'a> {
        BufferView {
            buffer,
            width,
            height,
            row_offset: 0,
            col_offset: 0,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        self.buffer
    }

    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn col_offset(&self) -> usize {
        self.col_offset
    }

    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    pub fn visible_lines(&self) -> impl Iterator<Item = (usize, &[u8])> {
        self.buffer
            .lines_from(self.row_offset)
            .enumerate()
            .map(|(i, line)| {
                let line: &[u8] = match self.col_offset < line.len() {
                    true => Some(&line[self.col_offset..]),
                    false => None,
                }
                .map(|line| &line[..cmp::min(self.width, line.len())])
                .unwrap_or(&[]);

                (i + self.row_offset, line)
            })
            .take(self.height)
    }

    pub fn set_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.row_offset =
            cmp::min(self.row_offset + lines, self.max_row_offset());
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.row_offset = self.row_offset.saturating_sub(lines);
    }

    pub fn scroll_right(&mut self, cols: usize) {
        self.col_offset =
            cmp::min(self.col_offset + cols, self.max_col_offset());
    }

    pub fn scroll_left(&mut self, cols: usize) {
        self.col_offset = self.col_offset.saturating_sub(cols);
    }

    pub fn scroll_to_row_begin(&mut self) {
        self.row_offset = 0;
    }

    pub fn scroll_to_col_begin(&mut self) {
        self.col_offset = 0;
    }

    pub fn scroll_to_row_end(&mut self) {
        self.row_offset = self.max_row_offset();
    }

    pub fn scroll_to_col_end(&mut self) {
        self.col_offset = self.max_col_offset();
    }

    fn max_row_offset(&self) -> usize {
        self.line_count().saturating_sub(self.height)
    }
    fn max_col_offset(&self) -> usize {
        self.buffer.max_line_len.saturating_sub(self.width)
    }
}
