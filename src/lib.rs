use std::fs::{self, File};
use std::io::{self, BufRead, Read};
use std::path::PathBuf;

use clap::Parser;

use crossterm::style::{ Stylize , Color};

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

fn read_stdin() -> io::Result<Vec<String>> {
    let reader = io::stdin().lock();
    reader.lines().collect()
}

fn print_content(config: &Config) -> io::Result<()> {
    if let Some(path) = &config.file_path {
        let mut file: File = fs::File::open(path.as_path())?;
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();

        println!("File content:");
        println!("{content}");
        return Ok(());
    }

    let buf = read_stdin()?;
    for line in buf {
        println!("{line}");
    }
    Ok(())
}

pub fn print_error(message: &str) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> io::Result<()> {
    let config = Config::parse();
    let term = Terminal::init()?;

    tui::print_events()?;

    //print_content(&config)
    Ok(())
}
