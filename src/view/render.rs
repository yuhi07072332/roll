use std::{borrow::Cow, ops::Range, string::FromUtf8Error};

use super::buffer::{Buffer};

pub struct RenderLineConfig<'a> {
    tab_stop: usize,
    highlights: &'a [Range<usize>]
}

pub struct RenderedLine {
    data: String,
    display_width: usize,
}

impl RenderedLine {
    fn new(line: &[u8], config: RenderLineConfig) 
    -> Result<RenderedLine, FromUtf8Error>  {
        let data = render_raw_line(line, config);
        let data = String::from_utf8(data)?;

        Ok(RenderedLine {
            data,
            display_width: 0
        })
    }
}

#[allow(clippy::same_item_push)]
fn render_raw_line(line: &[u8], config: RenderLineConfig) -> Vec<u8> {
    let RenderLineConfig{ tab_stop, highlights } = config;

    let mut rendered: Vec<u8> = Vec::with_capacity(line.len());
    let hl_index = 0;

    for (i, c) in line.iter().enumerate() {
        if let Some(hl) = highlights.get(hl_index) {
            if i == hl.start {
                rendered.extend_from_slice(b"\x1b[7m");
            } else if i == hl.end {
                rendered.extend_from_slice(b"\x1b[m");
            }
        }

        match c {
            b'\t' => {
                let spaces = tab_stop - (i % tab_stop);
                for _ in 0..spaces {
                    rendered.push(b' ');
                }
            }
            _ => rendered.push(*c)
        }
    }

    rendered
}

pub struct RenderCache{
    rlines: Vec<RenderedLine>,
    start_line: usize,
}


