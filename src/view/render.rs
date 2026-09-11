use std::ops::Range;

use super::buffer::Buffer;

#[derive(Debug, Clone)]
pub struct RenderLineConfig {
    pub tab_stop: usize,
}

impl Default for RenderLineConfig {
    fn default() -> Self {
        RenderLineConfig {
            tab_stop: 4,
        }
    }
}
#[derive(Debug)]
pub struct RenderedLine {
    pub data: String,
    pub line_number: usize,
}

impl RenderedLine {
    pub fn new(
        line: &[u8],
        line_number: usize,
        config: &RenderLineConfig,
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

        RenderedLine {
            data,
            line_number,
        }
    }

}

pub fn raw_index(rendered_byte_index: usize, raw: &[u8], rl_config: &RenderLineConfig) -> usize {
    let tab_stop = rl_config.tab_stop;
    let mut rx = 0;
    for (i, c) in raw.iter().enumerate() {
        match *c {
            b'\t' => {
                rx += tab_stop - (rx % tab_stop);
            }
            _ => rx += 1
        }
        if rx > rendered_byte_index {
            return i;
        }
    }

    raw.len()
}


#[allow(clippy::same_item_push)]
fn render_raw_line(line: &[u8], config: &RenderLineConfig) -> Vec<u8> {
    let RenderLineConfig {
        tab_stop,
    } = config;

    let mut rendered: Vec<u8> = Vec::with_capacity(line.len());

    let mut rx = 0;
    for c in line.iter() {
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
        };

        let s1 = b"\t";
        let s2 = b" \t";
        let s3 = b"  \t";
        let s4 = b"   \t";
        let s5 = b"\t\tstd::cout\t<< 42";
        assert_eq!(render_raw_line(s1, &config), b"    ");
        assert_eq!(render_raw_line(s2, &config), b"    ");
        assert_eq!(render_raw_line(s3, &config), b"    ");
        assert_eq!(render_raw_line(s4, &config), b"    ");
        assert_eq!(
            render_raw_line(s5, &config),
            b"        std::cout   << 42"
        );
    }

    #[test]
    fn raw_index_computes_rx_to_cx() {
        let raw = b"hello\tworld";
        let rl_config = RenderLineConfig { tab_stop: 4};
        let rline = RenderedLine::new(
            raw,
            1,
            &rl_config
        );

        assert_eq!(rline.data, "hello   world".to_string());
        assert_eq!(raw_index(3, raw, &rl_config), 3);
        assert_eq!(raw_index(5, raw, &rl_config), 5);
        assert_eq!(raw_index(6, raw, &rl_config), 5);
        assert_eq!(raw_index(7, raw, &rl_config), 5);
        assert_eq!(raw_index(8, raw, &rl_config), 6);
        assert_eq!(raw_index(9, raw, &rl_config), 7);
    }
}
