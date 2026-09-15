use std::{
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

    pub fn lines_in<'a>(&'a self, range: Range<usize>) -> LinesIter<'a> {
        LinesIter { buf: self, range }
    }

    pub fn lines<'a>(&'a self, from: usize, take: usize) -> LinesIter<'a> {
        self.lines_in(from..from + take)
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len().saturating_sub(1)
    }

    fn lines_from<'a>(&'a self, from: usize) -> LinesIter<'a> {
        self.lines_in(from..self.line_count())
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
#[derive(Debug)]
pub struct LinesIter<'a> {
    buf: &'a Buffer,
    range: Range<usize>,
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.range.is_empty() {
            return None;
        }

        let curr = self.range.start;
        let line = (curr, self.buf.line_at(curr)?);
        self.range.start += 1;
        Some(line)
    }
}

impl<'a> DoubleEndedIterator for LinesIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.range.is_empty() {
            return None;
        }

        self.range.end -= 1;
        let curr = self.range.end;
        let line = (curr, self.buf.line_at(curr)?);

        Some(line)
    }
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
