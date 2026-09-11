use std::{
    io::{self, Read, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    ops::Range
};

use anyhow::{ bail, Context };

use clap::Parser;

use crossterm::{
    event::{self, Event as TermEvent, MouseEvent, MouseEventKind},
    style::{Color, Stylize},
    tty::IsTty,
};

use input::{Key, SpecialKey::*};
use output::{RenderConfig, Renderer, SourceName, LineWrap};
use search::{SearchDirection, SearchState};
use terminal::{ScreenSize, TerminalGuard};
use view::{BYTES_PER_READ, Buffer, View, RenderedLine, RenderLineConfig, Line};

use crate::log::debug;

pub mod log;

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

    #[arg(short = 'w', long)]
    soft_wrap: bool,

    #[arg(short = 'W', long)]
    hard_wrap: bool,

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

struct Pager {
    buffer: Buffer,
    view: View,
    screen_size: ScreenSize,
    search_state: Option<SearchState>,

    rl_config: RenderLineConfig,
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
        let file_path_str = file_path.to_str().unwrap_or("(unknown)");
        source_name = SourceName::FileName(String::from(file_path_str));
        Buffer::from_file(file_path).context(format!("failed to read file: {}", file_path_str))?
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

    let wrap = if args.hard_wrap { LineWrap::HardWrap } 
        else if args.soft_wrap { LineWrap::SoftWrap}
        else { LineWrap::Disabled };

    let render_config = output::RenderConfig {
        line_numbers: args.line_numbers,
        source_name,
        wrap
    };

    let pager = Pager {
        buffer,
        view: View::new(),
        screen_size: terminal::size()?,
        search_state: None,
        rl_config: RenderLineConfig {
            tab_stop: 4
        }
    };

    run_loop(&args, render_config, pager, rx)?;

    Ok(())
}

fn run_loop(
    args: &Args,
    render_config: RenderConfig,
    mut pager: Pager,
    receiver: Receiver<Event>,
) -> anyhow::Result<()> {
    let terminal = TerminalGuard::init()?;
    let mut renderer = Renderer::new(render_config);
    let mut mode = Mode::Normal;

    let mut quit: bool = false;
    while !quit {
        renderer.resize_view(&mut pager);
        pager.view.ensure_visible_lines(&pager.buffer, &pager.rl_config);
        renderer.draw_frame(&pager, &mode)?;

        match receiver.recv()? {
            Event::BufferRead { n, bytes } => {
                pager.buffer.on_buffer_read(n, bytes);
            }
            Event::Terminal(TermEvent::Resize(width, height)) => {
                pager.screen_size = ScreenSize { width, height };
            }
            Event::Terminal(e @ TermEvent::Key(_))
            | Event::Terminal(e @ TermEvent::Mouse(_)) => {
                mode = handle_input(e, &mut pager, mode, || quit = true)?;
            }
            Event::BufferEof => {
                pager.buffer.on_buffer_eof();

                if args.quit_if_one_screen
                    && pager.buffer.line_count() < pager.view.height()
                {
                    drop(terminal);
                    print_buffer(&pager.buffer)?;
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
    pager: &mut Pager,
    mode: Mode,
    on_exit: impl FnOnce(),
) -> anyhow::Result<Mode> {
    match mode {
        Mode::Normal if pager.search_state.is_some() => {
            let mode = handle_normal_input(&event, pager, mode, on_exit);
            if let Mode::Normal = mode {
                handle_search_input(event, pager);
            }
            Ok(mode)
        }
        Mode::Normal => Ok(handle_normal_input(&event, pager, mode, on_exit)),
        Mode::Input(input_box) => {
            match handle_inputbox_input(event, input_box) {
                InputResult::Continue(input_box) => Ok(Mode::Input(input_box)),
                InputResult::Cancel => Ok(Mode::Normal),
                InputResult::Submit { input, action } => {
                    let result = on_input_submit(input, action, pager)?;
                    Ok(result)
                }
            }
        }
    }
}

fn on_input_submit(
    input: String,
    action: InputAction,
    pager: &mut Pager,
) -> anyhow::Result<Mode> {
    match action {
        InputAction::Search(direction) => {
            let mut state = SearchState::new(input, direction)?;
            state.search_from(pager.view.line_offset(), &pager.buffer)?;
            if let Some((n, _)) =
                state.next_match_from(pager.view.line_offset(), &pager.buffer)
            {
                pager.view.set_line_offset(*n, &pager.buffer);
            }
            pager.search_state = Some(state);
            Ok(Mode::Normal)
        }
    }
}

fn handle_normal_input(
    event: &TermEvent,
    pager: &mut Pager,
    mode: Mode,
    on_exit: impl FnOnce(),
) -> Mode {
    let Pager { buffer, view, .. } = pager;

    let half_page = view.height() as isize / 2;

    match *event {
        TermEvent::Key(key_event) => match Key::new(key_event) {
            Key::Char('q') => on_exit(),
            Key::Sp(Esc) if pager.search_state.is_none() => on_exit(),

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

fn handle_search_input(event: TermEvent, pager: &mut Pager) {
    let TermEvent::Key(key) = event else {
        return;
    };

    let Pager { view, buffer, .. } = pager;
    let state = pager.search_state.as_mut().unwrap();

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
        Key::Sp(Esc) => {
            pager.search_state = None;
        }
        _ => (),
    }
}
