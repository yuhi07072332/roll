use std::{cmp, collections::{BTreeMap, BTreeSet}, ops::{ Range, Bound} };

use crate::buffer::{Buffer, LinesIter};

use regex::Regex;

pub enum SearchDirection {
    Forward,
    Backward,
}

pub type SearchMatch = Vec<Range<usize>>;

pub struct SearchState {
    pattern: String,
    direction: SearchDirection,
    match_lines: BTreeMap<usize, SearchMatch>,
    coverage: ScanCoverage,
}

impl SearchState {
    pub fn new(pattern: String, direction: SearchDirection) -> SearchState {
        SearchState {
            pattern,
            direction,
            match_lines: BTreeMap::new(),
            coverage: ScanCoverage::default(),
        }
    }

    pub fn match_at(&self, line_number: usize) -> Option<&SearchMatch> {
        self.match_lines.get(&line_number)
    }

    pub fn search_from(
        &mut self,
        line_range: Range<usize>,
        buffer: &Buffer,
    ) {
        self.coverage.add(line_range);

        let mut line = line_range.start;
        while line < line_range.end {
            if let Some(uncovered) = 
                self.coverage.first_uncovered(line..line_range.end) {
                
            }
        }
    }
}

// A sorted, non-overlapping, merged range set.
// Ranges are represented as [key, value)
#[derive(Default)]
struct ScanCoverage(BTreeMap<usize, usize>);

impl ScanCoverage {
    fn first_uncovered(&self, range: Range<usize>) -> Option<Range<usize>> {
        let left_end = self
            .get_left(range.start)
            .map(|(_, end)| end)
            .unwrap_or(range.start);
        let right_start = self
            .get_right(range.start)
            .map(|(start, _)| start)
            .unwrap_or(range.end);

        let start = cmp::max(left_end, range.start);
        let end = cmp::min(right_start, range.end);

        (start < end).then_some(start..end)
    }

    fn is_full(&self, lines: usize) -> bool {
        self.0.len() == 1 && self.0.get(&0) == Some(&lines)
    }

    fn add(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }

        let Range{mut start, mut end} = range;

        if let Some((left_start, left_end)) = self.get_left(range.start)
            && range.start <= left_end {
            // merge range to left
            start = left_start;
            end = cmp::max(range.end, left_end);
            self.0.remove(&left_start);
        }

        while let Some((right_start, right_end)) = self.get_right(start) 
        && right_start <= end {
            // merge range to right
            end = cmp::max(end, right_end);
            self.0.remove(&right_start);
        }

        self.0.insert(start, end);
    }

    // find the first element in keys(..,range_start]
    fn get_left(&self, range_start: usize) -> Option<(usize, usize)> {
        self.0.range(..=range_start)
            .next_back()
            .map(|(l, r)| (*l, *r))
    }

    // find the first element in keys(range_start,..)
    fn get_right(&self, range_start: usize) -> Option<(usize, usize)> {
        self.0.range((Bound::Excluded(range_start), Bound::Unbounded))
            .next()
            .map(|(l, r)| (*l, *r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid(ranges: &ScanCoverage) {
        let mut prev_end = None;

        for (&start, &end) in &ranges.0 {
            assert!(start < end);

            if let Some(prev_end) = prev_end {
                assert!(start > prev_end);
            }

            prev_end = Some(end);
        }
    }

    #[test]
    fn add_to_scan_coverage() {
        let mut ranges = ScanCoverage::default();

        ranges.add(1..3);
        ranges.add(1..5);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&1), Some(&5));

        ranges.add(2..3);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&2), None);

        ranges.add(2..7);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&2), None);
        assert_eq!(ranges.0.get(&1), Some(&7));

        ranges.add(10..15);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&10), Some(&15));

        ranges.add(15..20);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&10), Some(&20));
        assert_eq!(ranges.0.get(&15), None);

        ranges.add(7..10);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&1), Some(&20));
        assert_eq!(ranges.0.get(&7), None);
        assert_eq!(ranges.0.get(&10), None);
        assert_eq!(ranges.0.get(&15), None);

        let mut ranges = ScanCoverage::default();
        ranges.add(2..3);
        ranges.add(5..7);
        ranges.add(9..11);
        ranges.add(3..10);
        assert_valid(&ranges);
        assert_eq!(ranges.0.get(&2), Some(&11));
        assert_eq!(ranges.0.get(&5), None);
        assert_eq!(ranges.0.get(&9), None);
    }

    #[test]
    fn first_uncovered() {
        let mut ranges = ScanCoverage::default();
        ranges.add(5..11);
        ranges.add(11..15);
        ranges.add(20..26);

        assert_valid(&ranges);

        assert_eq!(ranges.first_uncovered(5..7), None);
        assert_eq!(ranges.first_uncovered(7..13), None);
        assert_eq!(ranges.first_uncovered(7..15), None);
        assert_eq!(ranges.first_uncovered(20..26), None);

        assert_eq!(ranges.first_uncovered(3..10), Some(3..5));
        assert_eq!(ranges.first_uncovered(0..10), Some(0..5));

        assert_eq!(ranges.first_uncovered(13..17), Some(15..17));
        assert_eq!(ranges.first_uncovered(17..22), Some(17..20));
        assert_eq!(ranges.first_uncovered(13..22), Some(15..20));
        assert_eq!(ranges.first_uncovered(15..19), Some(15..19));

        assert_eq!(ranges.first_uncovered(2..21), Some(2..5));
        assert_eq!(ranges.first_uncovered(6..21), Some(15..20));

        assert_eq!(ranges.first_uncovered(27..30), Some(27..30));
    }

}
