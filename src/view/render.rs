
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub tab_stop: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig { tab_stop: 4 }
    }
}

#[derive(Debug)]
pub struct DisplayLine {
    pub data: String,
    pub line_number: usize,
}

impl DisplayLine {
    pub fn new(
        line: &[u8],
        line_number: usize,
        config: &RenderConfig,
    ) -> DisplayLine {
        let raw = expand_tab(line, config);
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

        DisplayLine { data, line_number }
    }
}

pub fn display_idx_to_raw_idx(
    display_idx: usize,
    raw: &[u8],
    config: &RenderConfig,
) -> usize {
    let tab_stop = config.tab_stop;
    let mut rx = 0;
    for (i, c) in raw.iter().enumerate() {
        match *c {
            b'\t' => {
                rx += tab_stop - (rx % tab_stop);
            }
            _ => rx += 1,
        }
        if rx > display_idx {
            return i;
        }
    }

    raw.len()
}

#[allow(clippy::same_item_push)]
fn expand_tab(line: &[u8], config: &RenderConfig) -> Vec<u8> {
    let RenderConfig { tab_stop } = config;

    let mut dline: Vec<u8> = Vec::with_capacity(line.len());

    let mut rx = 0;
    for c in line.iter() {
        match c {
            b'\t' => {
                let spaces = tab_stop - (rx % tab_stop);
                for _ in 0..spaces {
                    dline.push(b' ');
                }
                rx += spaces - 1;
            }
            _ => dline.push(*c),
        }

        rx += 1;
    }

    dline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_tab_to_space() {
        let config = RenderConfig { tab_stop: 4 };

        let s1 = b"\t";
        let s2 = b" \t";
        let s3 = b"  \t";
        let s4 = b"   \t";
        let s5 = b"\t\tstd::cout\t<< 42";
        assert_eq!(expand_tab(s1, &config), b"    ");
        assert_eq!(expand_tab(s2, &config), b"    ");
        assert_eq!(expand_tab(s3, &config), b"    ");
        assert_eq!(expand_tab(s4, &config), b"    ");
        assert_eq!(expand_tab(s5, &config), b"        std::cout   << 42");
    }

    #[test]
    fn raw_index_computes_rx_to_cx() {
        let raw = b"hello\tworld";
        let rl_config = RenderConfig { tab_stop: 4 };
        let rline = DisplayLine::new(raw, 1, &rl_config);

        assert_eq!(rline.data, "hello   world".to_string());
        assert_eq!(display_idx_to_raw_idx(3, raw, &rl_config), 3);
        assert_eq!(display_idx_to_raw_idx(5, raw, &rl_config), 5);
        assert_eq!(display_idx_to_raw_idx(6, raw, &rl_config), 5);
        assert_eq!(display_idx_to_raw_idx(7, raw, &rl_config), 5);
        assert_eq!(display_idx_to_raw_idx(8, raw, &rl_config), 6);
        assert_eq!(display_idx_to_raw_idx(9, raw, &rl_config), 7);
    }
}
