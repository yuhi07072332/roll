use std::{cmp, fmt, fs::{self, File}, io::{self, BufRead, BufReader, Read}, path::Path};

type Line = Vec<u8>;

pub struct Buffer {
    data: Vec<Line>,
    is_reading: bool,
}
pub struct BufferView<'a> {
    pub buffer: &'a Buffer,

    row_offset: usize,
    col_offset: usize,
}

impl Buffer {
    pub fn from_file(path: &Path) -> io::Result<Buffer> {
        let file: File = fs::File::open(path)?;

        Ok(Buffer {
            data: read_lines(io::BufReader::new(file))?,
            is_reading: false,
        })
    }

    pub fn from_stdin() -> io::Result<Buffer> {
        // TODO: spawn a new thread to read stdin instead of blocking the main thread
        Ok(Buffer {
            data: read_lines(io::BufReader::new(io::stdin().lock()))?,
            is_reading: false
        })
    }

    pub fn lines(&self) -> usize { self.data.len() }
}

impl<'a> BufferView<'a> {
    pub fn new(buffer: &'a Buffer) -> BufferView {
        BufferView {
            buffer,
            row_offset: 0,
            col_offset: 0,
        }
    }

    pub fn row_offset(&self) -> usize {
        self.row_offset
    }

    pub fn col_offset(&self) -> usize {
        self.col_offset
    }

    pub fn len(&self) -> usize {
        self.buffer.lines()
    }

    pub fn lines_from_offset(&self, take: usize) 
    -> impl Iterator<Item = (usize, &Line)> {
        self.buffer.data.iter().enumerate().skip(self.row_offset).take(take)
    }


    pub fn scroll_down(&mut self, lines: usize) {
        self.row_offset = cmp::min(
            self.row_offset + lines,
            self.len().saturating_sub(1)
        );
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.row_offset = self.row_offset.saturating_sub(lines);
    }
}

fn read_lines(reader: impl BufRead) -> io::Result<Vec<Line>> {
    // OPTIMIZE: maybe

    reader
        .lines()
        .map(|line_result| { 
            line_result.map(|line| {
                Vec::from(line.as_bytes())
            })
        })
        .collect()
}
