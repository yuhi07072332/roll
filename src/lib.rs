use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::bail;

use clap::Parser;

use crossterm::{
    event::{self, Event as TermEvent, MouseEvent, MouseEventKind},
    style::{Color, Stylize},
    tty::IsTty,
};

use input::{Key, SpecialKey::*};
use output::{RenderConfig, Renderer, SourceName};
use search::{SearchDirection, SearchState};
use terminal::{ScreenSize, TerminalGuard};
use view::{BYTES_PER_READ, Buffer, View};

mod input;
mod output;
mod search;
mod terminal;
mod view;

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
    Input(InputBox),
    Search(SearchState),
}

impl Mode {
    fn is_normal(&self) -> bool {
        matches!(self, Mode::Normal)
    }
    fn is_input(&self) -> bool {
        matches!(self, Mode::Input(_))
    }
    fn is_search(&self) -> bool {
        matches!(self, Mode::Search(_))
    }
}

enum InputAction {
    Search(SearchDirection),
}

enum InputResult {
    Continue(InputBox),
    Cancel,
    Submit { input: String, action: InputAction },
}

struct InputBox {
    input: String,
    prefix: Option<String>,
    action: InputAction,
}

impl InputBox {
    fn new(action: InputAction) -> InputBox {
        InputBox {
            input: String::new(),
            prefix: None,
            action,
        }
    }

    fn prefix(self, prefix: String) -> Self {
        InputBox {
            input: self.input,
            prefix: Some(prefix),
            action: self.action,
        }
    }
}

pub fn print_error(message: impl std::fmt::Display) {
    eprintln!("{} {message}", "error:".with(Color::Red).bold());
}

fn print_buffer(buffer: &Buffer) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    for (_, line) in buffer.lines(0, buffer.line_count()) {
        stdout.write_all(line)?;
        stdout.write_all(b"\r\n")?;
    }
    Ok(())
}

pub fn run() -> anyhow::Result<()> {
    let args = Args::try_parse()?;

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
        Buffer::from_stdin()
    };

    thread::spawn(move || send_from_terminal_event(tx));

    let render_config = output::RenderConfig {
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
) -> anyhow::Result<()> {
    let terminal = TerminalGuard::init()?;
    let mut view = View::new();
    let mut renderer = Renderer::new(render_config);
    let mut screen_size = terminal::size()?;
    let mut mode = Mode::Normal;

    let mut needs_exit: bool = false;
    while !needs_exit {
        renderer.resize_view(&mut view, screen_size, &buffer);
        view.ensure_visible_lines(&buffer);
        renderer.draw_frame(&buffer, &view, &mode, screen_size)?;

        match receiver.recv()? {
            Event::BufferRead { n, bytes } => {
                buffer.on_buffer_read(n, bytes);
            }
            Event::Terminal(TermEvent::Resize(width, height)) => {
                screen_size = ScreenSize { width, height };
            }
            Event::Terminal(e @ TermEvent::Key(_))
            | Event::Terminal(e @ TermEvent::Mouse(_)) => {
                mode = handle_input(e, mode, &buffer, &mut view, || {
                    needs_exit = true
                })?;
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

fn send_from_stdin(tx: Sender<Event>) -> anyhow::Result<()> {
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

fn send_from_terminal_event(tx: Sender<Event>) -> anyhow::Result<()> {
    loop {
        tx.send(Event::Terminal(event::read()?))?;
    }
}

fn handle_input(
    event: TermEvent,
    mode: Mode,
    buffer: &Buffer,
    view: &mut View,
    on_exit: impl FnOnce(),
) -> anyhow::Result<Mode> {
    match mode {
        Mode::Normal => {
            Ok(handle_normal_input(mode, &event, buffer, view, on_exit))
        }
        Mode::Input(input_box) => {
            match handle_inputbox_input(event, input_box) {
                InputResult::Continue(input_box) => Ok(Mode::Input(input_box)),
                InputResult::Cancel => Ok(Mode::Normal),
                InputResult::Submit { input, action } => {
                    let result = on_input_submit(input, action, buffer, view)?;
                    Ok(result)
                }
            }
        }
        Mode::Search(_) => {
            let mode = handle_normal_input(mode, &event, buffer, view, on_exit);
            if let Mode::Search(search_state) = mode {
                Ok(handle_search_input(event, search_state, buffer, view))
            } else {
                Ok(mode)
            }
        }
    }
}

fn on_input_submit(
    input: String,
    action: InputAction,
    buffer: &Buffer,
    view: &mut View,
) -> anyhow::Result<Mode> {
    match action {
        InputAction::Search(direction) => {
            let mut state = SearchState::new(input, direction)?;
            state.search_from(view.line_offset(), buffer)?;
            if let Some((n, _)) =
                state.next_match_from(view.line_offset(), buffer)
            {
                view.set_line_offset(*n, buffer);
            }
            Ok(Mode::Search(state))
        }
    }
}

fn handle_normal_input(
    mode: Mode,
    event: &TermEvent,
    buffer: &Buffer,
    view: &mut View,
    on_exit: impl FnOnce(),
) -> Mode {
    let half_page = view.height() as isize / 2;

    match *event {
        TermEvent::Key(key_event) => match Key::new(key_event) {
            Key::Char('q') => on_exit(),
            Key::Sp(Esc) if mode.is_normal() => on_exit(),

            Key::Char('j') | Key::Sp(Down) | Key::Sp(Enter) => {
                view.scroll_lines_clamp(1, buffer)
            }

            Key::Char('k') | Key::Sp(Up) => view.scroll_lines_clamp(-1, buffer),

            Key::Char('l') | Key::Sp(Right) => view.scroll_cols_clamp(1),
            Key::Char('h') | Key::Sp(Left) => view.scroll_cols_clamp(-1),

            Key::CtrlChar('d') | Key::Sp(PageDown) => {
                view.scroll_lines_clamp(half_page, buffer)
            }
            Key::CtrlChar('u') | Key::Sp(PageUp) => {
                view.scroll_lines_clamp(-half_page, buffer)
            }

            Key::Sp(Home) => view.set_col_offset(0),
            Key::Sp(End) => view.scroll_to_col_end(),
            Key::Char('g') => view.set_line_offset(0, buffer),
            Key::Char('G') => view.scroll_to_line_end(buffer),

            Key::Char('/') => {
                return Mode::Input(
                    InputBox::new(InputAction::Search(
                        SearchDirection::Forward,
                    ))
                    .prefix(String::from("/")),
                );
            }

            Key::Char('?') => {
                return Mode::Input(
                    InputBox::new(InputAction::Search(
                        SearchDirection::Backward,
                    ))
                    .prefix(String::from("?")),
                );
            }

            _ => (),
        },

        TermEvent::Mouse(MouseEvent { kind, .. }) => match kind {
            MouseEventKind::ScrollDown => view.scroll_lines_clamp(3, buffer),
            MouseEventKind::ScrollUp => view.scroll_lines_clamp(-3, buffer),
            _ => (),
        },
        _ => (),
    }

    mode
}

fn handle_inputbox_input(
    event: TermEvent,
    mut input_box: InputBox,
) -> InputResult {
    let TermEvent::Key(key) = event else {
        return InputResult::Continue(input_box);
    };

    match Key::new(key) {
        Key::Sp(Enter) => {
            return InputResult::Submit {
                input: input_box.input,
                action: input_box.action,
            };
        }
        Key::Sp(Esc) => return InputResult::Cancel,
        Key::Sp(Backspace) => {
            if input_box.input.pop().is_none() {
                return InputResult::Cancel;
            }
        }
        Key::Char(c) => {
            input_box.input.push(c);
        }
        _ => (),
    }

    InputResult::Continue(input_box)
}

fn handle_search_input(
    event: TermEvent,
    mut state: SearchState,
    buffer: &Buffer,
    view: &mut View,
) -> Mode {
    let TermEvent::Key(key) = event else {
        return Mode::Search(state);
    };
    match Key::new(key) {
        Key::Char('n') => {
            if let Some((n, _)) =
                state.next_match_from(view.line_offset(), buffer)
            {
                view.set_line_offset(*n, buffer);
            }
        }
        Key::Char('p') => {
            if let Some((n, _)) =
                state.prev_match_from(view.line_offset(), buffer)
            {
                view.set_line_offset(*n, buffer);
            }
        }
        Key::Sp(Esc) => return Mode::Normal,
        _ => (),
    }

    Mode::Search(state)
}
