use std::{cmp, collections::VecDeque, ops::Range};

pub use buffer::{BYTES_PER_READ, Buffer, Line, LinesIter};
pub use render::{ RenderedLine, RenderLineConfig };

mod buffer;
mod render;


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
    const CAPACITY: usize = 512;

    pub fn new() -> RenderCache {
        RenderCache {
            rlines: VecDeque::with_capacity(Self::CAPACITY),
        }
    }

    pub fn lines(&self, from: usize, take: usize) -> impl Iterator<Item = &RenderedLine> {
        self.rlines.iter()
            .skip_while(move |line| { line.line_number < from })
            .take(take)
    }

    pub fn ensure_lines(&mut self, visible: Range<usize>, buffer: &Buffer) {
        assert!(visible.end <= buffer.line_count());

        let range = self.range().unwrap_or_default();
        eprintln!("cache: {:?} ({} lines)", range.clone(), range.len());
        if range.contains_range(&visible) {
            return;
        }
        eprint!("visible: {:?} -> ", visible.clone());

        let new_range = desired_cache_range(visible, buffer.line_count());
        if range.contains_range(&new_range) {
            return;
        }

        eprintln!("new_range: {:?} ({} lines)", new_range.clone(), new_range.len());

        self.relocate(new_range, buffer);
    }

    fn relocate(&mut self, new_range: Range<usize>, buffer: &Buffer) {
        // TODO: the case new_range > old_range
        let old_range = self.range().unwrap_or_default();
        if new_range.overlaps_right(&old_range) {
            let push_range = new_range.start..old_range.start;
            eprint!("add front: {:?}", push_range.clone());
            self.add_front(push_range, buffer);
        } else if new_range.overlaps_left(&old_range) {
            let push_range = old_range.end..new_range.end;
            eprint!("add back: {:?}", push_range.clone());
            self.add_back(push_range, buffer);
        } else {
            self.rlines.clear();
            eprint!("add from empty");
            for line in buffer.lines_in(new_range) {
                self.rlines.push_back(self.render_line(line));
            }
        }

        eprintln!(" -> {:?}\n", self.range().unwrap_or_default());
    }

    fn add_front(&mut self, range: Range<usize>, buffer: &Buffer) {
        for _ in 0..range.len() {
            self.rlines.pop_back();
        }
        for line in buffer.lines_in(range).rev() {
            self.rlines.push_front(self.render_line(line));
        }
    }

    fn add_back(&mut self, range: Range<usize>, buffer: &Buffer) {
        for _ in 0..range.len() {
            self.rlines.pop_front();
        }
        for line in buffer.lines_in(range) {
            self.rlines.push_back(self.render_line(line));
        }
    }

    fn render_line(&self, (line_number, text): Line<'_>) -> RenderedLine {
        RenderedLine::new(
            text,
            line_number,
            RenderLineConfig {
                tab_stop: 4,
                ..Default::default()
            }
        )
    }

    fn range(&self) -> Option<Range<usize>> {
        Some(self.rlines.front()?.line_number..self.rlines.back()?.line_number + 1)
    }
}

fn desired_cache_range(
    visible: Range<usize>,
    buf_line_count: usize,
) -> Range<usize> {
    let start = cmp::min(
        visible.end.saturating_sub(RenderCache::CAPACITY / 2),
        visible.start,
    );
    let right_remaining = RenderCache::CAPACITY
        .saturating_sub(visible.len())
        .saturating_sub(visible.start - start);
    let end = cmp::min(visible.end + right_remaining, buf_line_count);
    let left_remaining = (visible.end + right_remaining).saturating_sub(buf_line_count);
    start.saturating_sub(left_remaining)..end
}

trait RangeExt {
    fn overlaps_left(&self, rhs: &Range<usize>) -> bool;
    fn overlaps_right(&self, rhs: &Range<usize>) -> bool;
    fn contains_range(&self, rhs: &Range<usize>) -> bool;
}

impl RangeExt for Range<usize> {
    fn overlaps_left(&self, rhs: &Range<usize>) -> bool {
        !self.is_empty()
            && !rhs.is_empty()
            && self.start >= rhs.start
            && self.start < rhs.end
            && self.end > rhs.end
    }

    fn overlaps_right(&self, rhs: &Range<usize>) -> bool {
        !self.is_empty()
            && !rhs.is_empty()
            && self.end > rhs.start
            && self.end <= rhs.end
            && self.start < rhs.start
    }

    fn contains_range(&self, rhs: &Range<usize>) -> bool {
        !self.is_empty() && !rhs.is_empty() && self.start <= rhs.start && self.end >= rhs.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPACITY: usize = RenderCache::CAPACITY;

    #[test]
    fn cache_range() {
        //TODO: this test will not work if CAPACITY is not 512
        assert_eq!(desired_cache_range(0..60, 1024), 0..CAPACITY);
        assert_eq!(desired_cache_range(0..60, 400), 0..400);
        assert_eq!(desired_cache_range(100..700, 1024), 100..700);

        let r = desired_cache_range(462..572, 1024);
        assert_eq!(r, 828 - CAPACITY..828);

        let r = desired_cache_range(462..572, 700);
        assert_eq!(r, 700 - CAPACITY..700);

        let r = desired_cache_range(690..700, 700);
        assert_eq!(r, 700 - CAPACITY..700);
    }
}
