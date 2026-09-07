use std::{cmp, collections::VecDeque, ops::Range};

pub use buffer::{BYTES_PER_READ, Buffer, Line, LinesIter};
pub use render::{ RenderedLine, RenderLineConfig };

mod buffer;
mod render;

const RENDER_CACHE_CAPACITY: usize = 512;
pub struct View {
    width: usize,
    height: usize,

    line_offset: usize,
    col_offset: usize,

    max_line_width: usize, // longest line width in current view

    render_cache: RenderCache,
}

impl View {
    pub fn new() -> View {
        View {
            width: 0,
            height: 0,
            line_offset: 0,
            col_offset: 0,
            max_line_width: 0,
            render_cache: RenderCache::new(),
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

    pub fn visible_lines(
        &self,
    ) -> impl Iterator<Item = &RenderedLine> {
        self.render_cache.lines(self.line_offset, self.height)
    }

    pub fn ensure_visible_lines(&mut self, buffer: &Buffer) {
        self.render_cache.ensure_lines(
            self.line_offset..self.line_offset + self.height,
            buffer,
        );
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
        let visible_max_offset =
            buffer.line_count().saturating_sub(self.height);
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

struct RenderCache {
    rlines: VecDeque<RenderedLine>,
}

impl RenderCache {
    pub fn new() -> RenderCache {
        RenderCache {
            rlines: VecDeque::with_capacity(RENDER_CACHE_CAPACITY),
        }
    }

    pub fn lines(&self, from: usize, take: usize) -> impl Iterator<Item = &RenderedLine> {
        self.rlines.iter()
            .skip_while(move |line| { line.line_number < from })
            .take(take)
    }

    pub fn ensure_lines(&mut self, visible: Range<usize>, buffer: &Buffer) {
        const HALF_CAPACITY: usize = RENDER_CACHE_CAPACITY / 2;

        let mut left_range = None;
        let mut right_range = None;
        if let Some(cache_range) = self.cache_range() {
            left_range = visible
                .overlaps_right_only(&cache_range)
                .then_some(visible.start..cache_range.start);
            right_range = visible
                .overlaps_left_only(&cache_range)
                .then_some(cache_range.end..visible.end);
        }

        let mut left_remaining = 0;

        if let Some(range) = left_range {
            let start = range.start.saturating_sub(HALF_CAPACITY);
            left_remaining = HALF_CAPACITY.saturating_sub(range.start);
            self.add_front(start..range.end, buffer);
        }

        if let Some(range) = right_range {
            let end = range.end + HALF_CAPACITY + left_remaining;
            self.add_back(range.start..end, buffer);
        }
    }

    fn add_front(&mut self, line_range: Range<usize>, buffer: &Buffer) {
        self.add(line_range, buffer, VecDeque::push_front, VecDeque::pop_back)
    }

    fn add_back(&mut self, line_range: Range<usize>, buffer: &Buffer) {
        self.add(line_range, buffer, VecDeque::push_back, VecDeque::pop_front)
    }

    fn add(
        &mut self,
        line_range: Range<usize>,
        buffer: &Buffer,
        push_fn: fn(&mut VecDeque<RenderedLine>, RenderedLine),
        pop_fn: fn(&mut VecDeque<RenderedLine>) -> Option<RenderedLine>,
    ) {
        if line_range.is_empty() {
            return;
        }

        while self.rlines.len() + line_range.len() > RENDER_CACHE_CAPACITY {
            pop_fn(&mut self.rlines);
        }

        buffer.lines_in(line_range).rev().for_each(|(n, text)| {
            push_fn(
                &mut self.rlines,
                RenderedLine::new(text, n, RenderLineConfig::default()),
            )
        });
    }

    fn cache_range(&self) -> Option<Range<usize>> {
        Some(self.rlines.front()?.line_number..self.rlines.len())
    }
}

trait RangeExt {
    fn overlaps_left(&self, rhs: &Range<usize>) -> bool;
    fn overlaps_right(&self, rhs: &Range<usize>) -> bool;
    fn overlaps_left_only(&self, rhs: &Range<usize>) -> bool;
    fn overlaps_right_only(&self, rhs: &Range<usize>) -> bool;
    // fn contains_range(&self, rhs: &Range<usize>) -> bool;
}

impl RangeExt for Range<usize> {
    fn overlaps_left(&self, rhs: &Range<usize>) -> bool {
        !self.is_empty()
            && !rhs.is_empty()
            && self.start > rhs.start
            && self.end > rhs.end
    }

    fn overlaps_right(&self, rhs: &Range<usize>) -> bool {
        !self.is_empty()
            && !rhs.is_empty()
            && self.start < rhs.start
            && self.end < rhs.end
    }

    fn overlaps_left_only(&self, rhs: &Range<usize>) -> bool {
        self.overlaps_left(rhs) && !self.overlaps_right(rhs)
    }

    fn overlaps_right_only(&self, rhs: &Range<usize>) -> bool {
        self.overlaps_right(rhs) && !self.overlaps_left(rhs)
    }

    // fn contains_range(&self, rhs: &Range<usize>) -> bool {
    //     !self.is_empty() && !rhs.is_empty() && self.start <= rhs.start && self.end >= rhs.end
    // }
}
