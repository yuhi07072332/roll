mod render;
mod terminal;
mod buffer;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Stylize};

use render::{RenderConfig, Renderer};
use terminal::TerminalGuard;
use buffer::{Buffer, BufferView};

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


pub fn print_error(message: &str) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> io::Result<()> {
    let args = Args::parse();
    let render_config = render::RenderConfig {
        line_numbers: args.line_numbers,
    };

    let buffer: Buffer = if let Some(file_path) = args.file_path {
        Buffer::from_file(&file_path)?
    } else {
        Buffer::from_stdin()?
    };

    let mut view = BufferView::new(&buffer);

    run_loop(&mut view, render_config)?;

    Ok(())
}

fn run_loop(
    view: &mut BufferView,
    render_config: RenderConfig,
) -> io::Result<()> {
    let mut terminal = TerminalGuard::init()?;
    let mut renderer = Renderer::new(render_config, &terminal);

    loop {
        renderer.draw_frame(view, &terminal)?;

        if event::poll(Duration::from_millis(250))?
            && handle_event(event::read()?, view, &mut terminal)
        {
            break;
        }
    }

    Ok(())
}

/// # Returns
/// true if needs to quit
#[allow(clippy::match_like_matches_macro)]
fn handle_event(
    event: Event,
    view: &mut BufferView,
    terminal: &mut TerminalGuard,
) -> bool {
    fn is_ctrl(key: KeyEvent) -> bool {
        key.modifiers == KeyModifiers::CONTROL
    }

    let half_page = terminal.height() as usize / 2;

    match event {
        Event::Key(key @ KeyEvent {
            code, ..
        }) => match code {
            // quit
            KeyCode::Esc => return true,

            // move one file line
            KeyCode::Char('j') | KeyCode::Down 
                | KeyCode::Enter
                =>
            {
                view.scroll_down(1);
            },
            KeyCode::Char('k') | KeyCode::Up => {
                view.scroll_up(1);
            },

            // move half page
            KeyCode::Char('d') if is_ctrl(key) => {
                view.scroll_down(half_page);
            },
            KeyCode::PageDown => view.scroll_down(half_page),
            KeyCode::Char('u') if is_ctrl(key) => {
                view.scroll_up(half_page);
            }
            KeyCode::PageUp => view.scroll_up(half_page),

            _ => (),
        },

        // 我承认这个确实有点蠢
        event @ Event::Resize(_, _) => {
            terminal.on_event(event);
        }
        _ => (),
    }

    false
}
