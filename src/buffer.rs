use std::{
    cmp,
    fs::{self},
    io::{self, Read},
    path::Path,
};

#[derive(Default)]
pub struct Buffer {
    data: Vec<u8>,
    line_starts: Vec<usize>,
    is_reading: bool,
}

impl Buffer {
    pub fn from_file(path: &Path) -> io::Result<Buffer> {
        let data = fs::read(path)?;
        let line_starts = scan_line_starts(&data);

        Ok(Buffer {
            data,
            line_starts,
            is_reading: false,
        })
    }

    pub fn from_stdin() -> io::Result<Buffer> {
        // TODO: spawn a new thread to read stdin instead of blocking the main thread
        let mut data: Vec<u8> = Vec::new();
        io::stdin().lock().read_to_end(&mut data)?;
        let line_starts = scan_line_starts(&data);

        Ok(Buffer {
            data,
            line_starts,
            is_reading: false,
        })
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

    pub fn lines(&self, from: usize, take: usize) -> LinesIter {
        LinesIter {
            buf: self,
            current_index: from,
            last_index: from + take
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len() - 1
    }
}

fn scan_line_starts(data: &[u8]) -> Vec<usize> {
    let mut line_starts = vec![0];

    line_starts.extend(
        data.iter()
            .enumerate()
            .filter_map(|(idx, c)| (*c == b'\n').then_some(idx + 1)),
    );

    if let Some(c) = data.last()
        && *c != b'\n'
    {
        line_starts.push(data.len());
    }

    line_starts
}

/// similar to `BufRead::lines()`
pub struct LinesIter<'a> {
    buf: &'a Buffer,
    current_index: usize,
    last_index: usize
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index > self.last_index { return None; }

        let line = (self.current_index, self.buf.line_at(self.current_index)?);
        self.current_index += 1;
        Some(line)
    }
}


pub struct BufferView<'a> {
    buffer: &'a Buffer,
    width: usize,
    height: usize,

    row_offset: usize,
    col_offset: usize,

    max_line_width: usize, // longest line width in current view
}

impl<'a> BufferView<'a> {
    pub fn new(
        buffer: &'a Buffer,
        width: usize,
        height: usize,
    ) -> BufferView<'a> {
        let mut view = BufferView {
            buffer,
            width,
            height,
            row_offset: 0,
            col_offset: 0,
            max_line_width: 0,
        };
        view.update_max_line_width(buffer.lines(0, height));
        view
    }

    pub fn buffer(&self) -> &Buffer {
        self.buffer
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

    pub fn col_offset(&self) -> usize {
        self.col_offset
    }

    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    pub fn visible_lines(&self) -> impl Iterator<Item = (usize, &[u8])> {
        self.buffer
            .lines(self.row_offset, self.height)
            .map(|(index, line)| {
                let line = if self.col_offset < line.len() {
                    &line[self.col_offset..]
                } else { &[] };

                (index, line)
            })
    }

    pub fn set_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.set_row_offset(self.row_offset);
        self.set_col_offset(self.col_offset);
    }

    pub fn set_row_offset(&mut self, offset: usize) {
        let offset = self.clamp_row_offset(offset);
        if offset != self.row_offset {
            self.update_max_line_width(self.buffer.lines(offset, self.height));
        }
        self.row_offset = offset;
    }

    pub fn set_col_offset(&mut self, offset: usize) {
        self.col_offset = self.clamp_col_offset(offset);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.set_row_offset(self.row_offset + lines);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.set_row_offset(self.row_offset.saturating_sub(lines));
    }

    pub fn scroll_right(&mut self, cols: usize) {
        self.set_col_offset(self.col_offset + cols);
    }

    pub fn scroll_left(&mut self, cols: usize) {
        self.set_col_offset(self.col_offset.saturating_sub(cols));
    }

    pub fn scroll_to_row_end(&mut self) {
        self.row_offset = self.line_count().saturating_sub(self.height);
    }

    pub fn scroll_to_col_end(&mut self) {
        self.col_offset = self.max_line_width.saturating_sub(self.width);
    }

    fn clamp_row_offset(&self, offset: usize) -> usize {
        let max_offset = self.line_count().saturating_sub(self.height);
        cmp::min(offset, max_offset)
    }

    fn clamp_col_offset(&self, offset: usize) -> usize {
        let max_offset = self.max_line_width.saturating_sub(self.width);
        cmp::min(offset, max_offset)
    }

    fn update_max_line_width(& mut self, lines: LinesIter<'a>) {
        self.max_line_width = lines
            .map(|(_, line)| line.len())
            .max()
            .unwrap_or(0)
    }
}
