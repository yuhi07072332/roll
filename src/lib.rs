mod buffer;
mod render;
mod terminal;

use std::{
    io::{self, Read, Write}, ops::Deref, path::PathBuf, sync::mpsc::{self, Receiver, Sender}, thread
};

use anyhow::{Result, bail};

use clap::Parser;

use crossterm::{
    event::{
        self, Event as TermEvent, KeyCode, KeyEvent, KeyModifiers, MouseEvent,
        MouseEventKind,
    },
    style::{Color, Stylize},
};

use buffer::{BYTES_PER_READ, Buffer, BufferView};
use crossterm::tty::IsTty;
use render::{RenderConfig, Renderer, SourceName};
use terminal::{ScreenSize, TerminalGuard};

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

enum Event {
    Terminal(TermEvent),
    BufferRead { n: usize, bytes: Box<[u8]> },
    BufferEof,
}
enum Mode {
    Normal,
    Input(InputBox)
}

struct InputBox {
    input: String,
    on_enter: Box<dyn FnOnce(&str)>
}

impl InputBox {
    fn new(on_enter: Box<dyn FnOnce(&str)>) -> InputBox {
        InputBox {
            input: String::new(),
            on_enter
        }
    }

    fn on_enter(self) {
        let callback = self.on_enter;
        callback(&self.input)
    }
}

pub fn print_error(message: impl std::fmt::Display) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    let (tx, rx) = mpsc::channel::<Event>();

    let source_name;

    let buffer = if let Some(file_path) = &args.file_path {
        source_name = SourceName::FileName(String::from(
            file_path.to_str().unwrap_or("(unknown)"),
        ));
        Buffer::from_file(file_path)?
    } else {
        if io::stdin().is_tty() {
            bail!("missing file or piped stdin");
        }
        source_name = SourceName::Stdin;
        let stdin_sender = tx.clone();
        thread::spawn(move || send_from_stdin(stdin_sender));
        Buffer::from_stdin()?
    };

    thread::spawn(move || send_from_terminal_event(tx));

    let render_config = render::RenderConfig {
        line_numbers: args.line_numbers,
        source_name,
    };

    run_loop(&args, buffer, render_config, rx)?;

    Ok(())
}

fn run_loop(
    args: &Args,
    mut buffer: Buffer,
    render_config: RenderConfig,
    receiver: Receiver<Event>,
) -> Result<()> {
    let terminal = TerminalGuard::init()?;
    let mut view = BufferView::new();
    let mut renderer = Renderer::new(render_config);
    let mut screen_size = terminal::size()?;
    let mut mode = Mode::Normal;

    let mut needs_exit: bool = false;
    while !needs_exit {
        renderer.resize_view(&mut view, screen_size, &buffer);
        renderer.draw_frame(&buffer, &view, &mode, screen_size)?;

        match receiver.recv()? {
            Event::BufferRead { n, bytes } => {
                buffer.on_buffer_read(n, bytes);
            }
            Event::Terminal(TermEvent::Resize(width, height)) => {
                screen_size = ScreenSize(width, height);
            }
            Event::Terminal(e @ TermEvent::Key(_))
            | Event::Terminal(e @ TermEvent::Mouse(_)) => {
                mode = handle_terminal_input(
                    e,
                    mode,
                    &mut buffer,
                    &mut view,
                    || needs_exit = true,
                );
            }
            Event::BufferEof => {
                buffer.on_buffer_eof();

                if args.quit_if_one_screen
                    && buffer.line_count() < view.height()
                {
                    drop(terminal);
                    print_buffer(&buffer)?;
                    return Ok(());
                }
            }
            _ => (),
        }
    }

    Ok(())
}

fn send_from_stdin(tx: Sender<Event>) -> Result<()> {
    let mut stdin = io::stdin().lock();

    loop {
        let mut buf = [0u8; BYTES_PER_READ];
        let n = stdin.read(&mut buf)?;

        if n == 0 {
            break;
        }
        tx.send(Event::BufferRead {
            n,
            bytes: Box::new(buf),
        })?;
    }

    tx.send(Event::BufferEof)?;
    Ok(())
}

fn send_from_terminal_event(tx: Sender<Event>) -> Result<()> {
    loop {
        tx.send(Event::Terminal(event::read()?))?;
    }
}

fn handle_terminal_input(
    event: TermEvent,
    mode: Mode,
    buffer: &mut Buffer,
    view: &mut BufferView,
    on_exit: impl FnOnce(),
) -> Mode {
    match mode {
        Mode::Normal => handle_terminal_normal_input(event, buffer, view, on_exit),
        Mode::Input(input_box) => handle_terminal_inputbox(event, input_box),
    }
}

fn handle_terminal_normal_input(
    event: TermEvent,
    buffer: &mut Buffer,
    view: &mut BufferView,
    on_exit: impl FnOnce(),
) -> Mode {
    let half_page = view.height() / 2;

    match event {
        TermEvent::Key(KeyEvent {
            code, modifiers, ..
        }) if modifiers == KeyModifiers::NONE
            || modifiers == KeyModifiers::SHIFT =>
        {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => on_exit(),

                KeyCode::Char('j') | KeyCode::Down | KeyCode::Enter => {
                    view.scroll_down(1, buffer)
                }
                KeyCode::Char('k') | KeyCode::Up => view.scroll_up(1, buffer),

                KeyCode::Char('l') | KeyCode::Right => view.scroll_right(1),
                KeyCode::Char('h') | KeyCode::Left => view.scroll_left(1),

                KeyCode::PageDown => view.scroll_down(half_page, buffer),
                KeyCode::PageUp => view.scroll_up(half_page, buffer),

                KeyCode::Home => view.set_col_offset(0),
                KeyCode::End => view.scroll_to_col_end(),
                KeyCode::Char('g') => view.set_row_offset(0, buffer),
                KeyCode::Char('G') => view.scroll_to_row_end(buffer),

                KeyCode::Char('/') => {
                    return Mode::Input(InputBox::new(Box::new(search_front)))
                }

                _ => (),
            }
        }

        // with control
        TermEvent::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            ..
        }) => match code {
            KeyCode::Char('d') => view.scroll_down(half_page, buffer),
            KeyCode::Char('u') => view.scroll_up(half_page, buffer),

            _ => (),
        },

        TermEvent::Mouse(MouseEvent { kind, .. }) => match kind {
            MouseEventKind::ScrollDown => view.scroll_down(3, buffer),
            MouseEventKind::ScrollUp => view.scroll_up(3, buffer),
            _ => (),
        },
        _ => (),
    }

    Mode::Normal
}

fn handle_terminal_inputbox(event: TermEvent, mut input_box: InputBox) -> Mode {
    let TermEvent::Key(key) = event else {
        return Mode::Input(input_box);
    };

    match key {
        KeyEvent{code, modifiers, ..} 
        if modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT => match code {
            KeyCode::Enter => {
                input_box.on_enter();
                return Mode::Normal;
            }
            KeyCode::Esc => {
                return Mode::Normal;
            }
            KeyCode::Backspace => {
                input_box.input.pop();
            }
            KeyCode::Char(c) if !c.is_control() => {
                input_box.input.push(c);
            }
            _ => ()
        }
        _ => ()
    }
    
    Mode::Input(input_box)
}

fn search_front(pattern: &str) {}

fn print_buffer(buffer: &Buffer) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    for (_, line) in buffer.lines(0, buffer.line_count()) {
        stdout.write_all(line)?;
        stdout.write_all(b"\r\n")?;
    }
    Ok(())
}
