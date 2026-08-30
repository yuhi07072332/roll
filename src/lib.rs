mod buffer;
mod render;
mod terminal;

use std::{io, path::PathBuf, time::Duration};

use anyhow::{Result, bail};

use clap::Parser;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Stylize},
};

use buffer::{Buffer, BufferView};
use crossterm::tty::IsTty;
use render::{RenderConfig, Renderer, ScreenSize};
use terminal::Terminal;

const FRAME_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Parser, Debug)]
#[command(name = "roll")]
#[command(version = "0.1.0")]
struct Args {
    #[arg(short = 'F', long)]
    quit_if_one_screen: bool, // TODO:

    #[arg(short = 'n', long)]
    line_numbers: bool,

    #[arg(id = "PATH/TO/FILE")]
    file_path: Option<PathBuf>,
}

pub fn print_error(message: impl std::fmt::Display) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> Result<()> {
    let args = Args::parse();
    let render_config = render::RenderConfig {
        line_numbers: args.line_numbers,
    };

    let buffer = if let Some(file_path) = args.file_path {
        Buffer::from_file(&file_path)?
    } else {
        if io::stdin().is_tty() {
            bail!("missing file or piped stdin");
        }
        Buffer::from_stdin()?
    };

    run_loop(&buffer, render_config)?;

    Ok(())
}

fn run_loop(buffer: &Buffer, render_config: RenderConfig) -> io::Result<()> {
    let mut terminal = Terminal::init()?;
    let mut view = BufferView::new(
        buffer,
        terminal.width as usize,
        terminal.height as usize,
    );
    let mut renderer = Renderer::new(render_config);
    let mut needs_exit: bool = false;

    while !needs_exit {
        let screen_size = ScreenSize(terminal.width, terminal.height);
        renderer.resize_view(&mut view, screen_size);
        renderer.draw_frame(&view, screen_size)?;

        if event::poll(FRAME_TIMEOUT)? {
            handle_event(event::read()?, &mut view, &mut terminal, || {
                needs_exit = true
            })
        }
    }

    Ok(())
}

fn handle_event(
    event: Event,
    view: &mut BufferView,
    terminal: &mut Terminal,
    on_exit: impl FnOnce(),
) {
    let half_page = view.height() / 2;

    match event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        }) => match code {
            KeyCode::Char('q') | KeyCode::Esc => on_exit(),

            KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => {
                view.scroll_down(1)
            }
            KeyCode::Char('k') | KeyCode::Up => view.scroll_up(1),

            KeyCode::Char('l') | KeyCode::Right => view.scroll_right(1),
            KeyCode::Char('h') | KeyCode::Left => view.scroll_left(1),

            KeyCode::PageDown => view.scroll_down(half_page),
            KeyCode::PageUp => view.scroll_up(half_page),

            KeyCode::Home => view.set_col_offset(0),
            KeyCode::End => view.scroll_to_col_end(),
            KeyCode::Char('g') => view.set_row_offset(0),
            KeyCode::Char('G') => view.scroll_to_row_end(),

            _ => (),
        },

        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => match code {
            KeyCode::Char('d') => view.scroll_down(half_page),
            KeyCode::Char('u') => view.scroll_up(half_page),

            _ => (),
        },

        Event::Resize(width, height) => {
            terminal.width = width;
            terminal.height = height;
        }
        _ => (),
    }
}
