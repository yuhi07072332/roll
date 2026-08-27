mod buffer;
mod render;
mod terminal;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Stylize};

use buffer::{Buffer, BufferView};
use render::{ScreenSize, RenderConfig, Renderer};
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

pub fn run() -> io::Result<()> {
    let args = Args::parse();
    let render_config = render::RenderConfig {
        line_numbers: args.line_numbers,
    };

    let buffer  = if let Some(file_path) = args.file_path {
        Buffer::from_file(&file_path)?
    } else {
        Buffer::from_stdin()?
    };

    let mut view = BufferView::new(&buffer);

    run_loop(&mut view, render_config)
}

fn run_loop(
    view: &mut BufferView,
    render_config: RenderConfig,
) -> io::Result<()> {
    let mut terminal = Terminal::init()?;
    let mut renderer = Renderer::new(render_config, );

    loop {
        renderer.draw_frame(view, ScreenSize(terminal.width, terminal.height))?;

        if event::poll(FRAME_TIMEOUT)?
            && handle_event(event::read()?, view, &mut terminal)
        {
            break;
        }
    }

    Ok(())
}

/// # Returns
/// true if needs to quit
fn handle_event(
    event: Event,
    view: &mut BufferView,
    terminal: &mut Terminal,
) -> bool {
    fn is_ctrl(key: KeyEvent) -> bool {
        key.modifiers == KeyModifiers::CONTROL
    }

    let half_page = terminal.height as usize / 2;

    match event {
        Event::Key(key @ KeyEvent { code, .. }) => match code {
            // quit
            KeyCode::Esc => return true,

            // move one file line
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => {
                view.scroll_down(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                view.scroll_up(1);
            }

            // move half page
            KeyCode::Char('d') if is_ctrl(key) => {
                view.scroll_down(half_page);
            }
            KeyCode::PageDown => view.scroll_down(half_page),
            KeyCode::Char('u') if is_ctrl(key) => {
                view.scroll_up(half_page);
            }
            KeyCode::PageUp => view.scroll_up(half_page),

            _ => (),
        },

        // 我承认这个确实有点蠢
        Event::Resize(width, height) => {
            terminal.width = width;
            terminal.height = height;
        }
        _ => (),
    }

    false
}
