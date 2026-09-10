use std::ops::Range;

use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct RenderLineConfig<'a> {
    pub tab_stop: usize,
    pub highlights: &'a [Range<usize>],
}

impl Default for RenderLineConfig<'_> {
    fn default() -> Self {
        RenderLineConfig {
            tab_stop: 4,
            highlights: Default::default(),
        }
    }
}
#[derive(Debug)]
pub struct RenderedLine {
    pub data: String,
    pub line_number: usize,
    pub display_width: usize,
}

impl RenderedLine {
    pub fn new(
        line: &[u8],
        line_number: usize,
        config: RenderLineConfig,
    ) -> RenderedLine {
        let raw = render_raw_line(line, config);
        let data = match String::from_utf8(raw) {
            Ok(text) => text,
            Err(err) => {
                let utf8_err = err.utf8_error();
                let mut raw = err.into_bytes();

                match utf8_err.error_len() {
                    None => {
                        raw.truncate(utf8_err.valid_up_to());
                        String::from_utf8(raw).unwrap()
                    }
                    Some(_) => String::from_utf8_lossy(&raw).into_owned(),
                }
            }
        };

        let display_width = data.width();

        RenderedLine {
            data,
            line_number,
            display_width,
        }
    }
}

#[allow(clippy::same_item_push)]
fn render_raw_line(line: &[u8], config: RenderLineConfig) -> Vec<u8> {
    let RenderLineConfig {
        tab_stop,
        highlights,
    } = config;

    let mut rendered: Vec<u8> = Vec::with_capacity(line.len());

    let (mut hl_index, mut rx) = (0, 0);
    for (x, c) in line.iter().enumerate() {
        if let Some(hl) = highlights.get(hl_index) {
            if x == hl.start {
                rendered.extend_from_slice(b"\x1b[7m");
            } else if x == hl.end {
                rendered.extend_from_slice(b"\x1b[m");
                hl_index += 1;
            }
        }

        match c {
            b'\t' => {
                let spaces = tab_stop - (rx % tab_stop);
                for _ in 0..spaces {
                    rendered.push(b' ');
                }
                rx += spaces - 1;
            }
            _ => rendered.push(*c),
        }

        rx += 1;
    }

    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_tab_to_space() {
        let config = RenderLineConfig {
            tab_stop: 4,
            ..Default::default()
        };

        let s1 = b"\t";
        let s2 = b" \t";
        let s3 = b"  \t";
        let s4 = b"   \t";
        let s5 = b"\t\tstd::cout\t<< 42";
        assert_eq!(render_raw_line(s1, config.clone()), b"    ");
        assert_eq!(render_raw_line(s2, config.clone()), b"    ");
        assert_eq!(render_raw_line(s3, config.clone()), b"    ");
        assert_eq!(render_raw_line(s4, config.clone()), b"    ");
        assert_eq!(
            render_raw_line(s5, config.clone()),
            b"        std::cout   << 42"
        );
    }

    #[test]
    fn render_highlight() {
        let highlights: Vec<Range<usize>> = Vec::from([1..5, 6..10]);

        let config = RenderLineConfig {
            highlights: &highlights,
            ..Default::default()
        };

        let s = b"lorem ipsum";
        assert_eq!(
            render_raw_line(s, config),
            b"l\x1b[7morem\x1b[m \x1b[7mipsu\x1b[mm"
        );
    }
}
