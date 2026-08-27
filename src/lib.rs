use std::cmp::{max, min};
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Stylize};

use crate::tui::{RenderConfig, Terminal};

mod tui;

#[derive(Parser, Debug)]
#[command(name = "roll")]
#[command(version = "0.1.0")]
struct Args {
    #[arg(short = 'F', long)]
    quit_if_one_screen: bool,

    #[arg(short = 'n', long)]
    line_numbers: bool,

    #[arg(id = "PATH/TO/FILE")]
    file_path: Option<PathBuf>,
}

type Buffer = Vec<String>;

fn read_from_stdin() -> io::Result<Buffer> {
    let reader = io::stdin().lock();
    reader.lines().collect()
}

fn read_from_file(path: &Path) -> io::Result<Buffer> {
    let file: File = fs::File::open(path)?;
    io::BufReader::new(file).lines().collect()
}

struct ContentView {
    buffer: Buffer,
    row_offset: usize,
}

impl ContentView {
    pub fn new(buffer: Buffer) -> ContentView {
        ContentView {
            buffer,
            row_offset: 0,
        }
    }
}

pub fn print_error(message: &str) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> io::Result<()> {
    let args = Args::parse();
    let render_config = tui::RenderConfig {
        line_numbers: args.line_numbers,
    };

    let buffer: Buffer = if let Some(file_path) = args.file_path {
        read_from_file(&file_path)?
    } else {
        read_from_stdin()?
    };

    let mut content_view = ContentView::new(buffer);
    run_loop(&mut content_view, &render_config)?;

    Ok(())
}

fn run_loop(content: &mut ContentView, render_config: &RenderConfig) -> io::Result<()> {
    let mut term = Terminal::init()?;
    let mut stdout = io::stdout();

    loop {
        tui::clear_screen(&mut stdout)?;
        tui::draw_content(&mut stdout, &term, content, render_config)?;
        tui::draw_status_line(&mut stdout, &term, content)?;

        stdout.flush()?;

        if event::poll(Duration::from_millis(250))?
            && handle_event(event::read()?, content, &mut term)
        {
            break;
        }
    }

    Ok(())
}

/// # Returns
/// true if needs to quit
#[allow(clippy::match_like_matches_macro)]
fn handle_event(event: Event, content_view: &mut ContentView, terminal: &mut Terminal) -> bool {
    let file_rows = content_view.buffer.len();
    let row_offset = &mut content_view.row_offset;

    let page_down = |row_offset: usize| {
        min(
            row_offset + (terminal.height() / 2) as usize,
            content_view.buffer.len(),
        )
    };

    let page_up = |row_offset: usize| row_offset.saturating_sub((terminal.height() / 2) as usize);

    match event {
        Event::Key(KeyEvent {
            code, modifiers, ..
        }) => match code {
            KeyCode::Esc => return true,

            KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter if *row_offset < file_rows => {
                content_view.row_offset += 1;
            }
            KeyCode::Char('k') | KeyCode::Up if *row_offset > 0 => {
                content_view.row_offset -= 1;
            }

            KeyCode::Char('d') if modifiers == KeyModifiers::CONTROL => {
                *row_offset = page_down(*row_offset)
            }
            KeyCode::PageDown => *row_offset = page_down(*row_offset),
            KeyCode::Char('u') if modifiers == KeyModifiers::CONTROL => {
                *row_offset = page_up(*row_offset)
            }
            KeyCode::PageUp => *row_offset = page_up(*row_offset),

            _ => (),
        },
        Event::Resize(width, height) => {
            terminal.resize(width, height);
        }
        _ => (),
    }

    false
}
