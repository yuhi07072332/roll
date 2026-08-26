use std::fs::{self, File};
use std::io::{self, BufRead, Read, Write, stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Color, Stylize};

use crate::tui::Terminal;

mod tui;

#[derive(Parser, Debug)]
#[command(name = "roll")]
#[command(version = "0.1.0")]
struct Config {
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

fn run_loop(content: &ContentView) -> io::Result<()> {
    let term = Terminal::init()?;
    let mut stdout = io::stdout();

    loop {
        if event::poll(Duration::from_millis(250))? {
            if handle_event(event::read()?) {
                break;
            }
        }

        tui::draw_content(&mut stdout, &term, &content)?;
        tui::draw_status_line(&mut stdout, &term, &content)?;

        stdout.flush()?;
    }

    Ok(())
}

/// # Returns
/// true if needs to quit
#[allow(clippy::match_like_matches_macro)]
fn handle_event(event: Event) -> bool {
    match event {
        Event::Key(KeyEvent { code, .. }) => match code {
            KeyCode::Esc => true,
            _ => false,
        },
        _ => false,
    }
}

pub fn print_error(message: &str) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> io::Result<()> {
    let config = Config::parse();

    let buffer: Buffer = if let Some(file_path) = config.file_path {
        read_from_file(&file_path)?
    } else {
        read_from_stdin()?
    };

    let content_view = ContentView::new(buffer);
    run_loop(&content_view)?;

    Ok(())
}
