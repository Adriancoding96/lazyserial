use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{execute, terminal};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::serial::{self, SerialEvent, SerialHandle};
use crate::ui;

const MAX_OUTPUT_LINES: usize = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Ports,
    Output,
    Input,
}

pub struct AppState {
    pub ports: Vec<serialport::SerialPortInfo>,
    pub selected_port: Option<usize>,
    pub baud_rate: u32,
    pub is_open: bool,

    pub serial_handle: Option<SerialHandle>,
    pub serial_event_rx: Option<std::sync::mpsc::Receiver<SerialEvent>>,

    pub output_lines: VecDeque<String>,
    pub output_scroll: u16,

    pub input_buffer: String,
    pub focus: Focus,
}

impl AppState {
    fn new() -> Result<Self> {
        let ports = serial::list_ports()?;
        Ok(Self {
            ports,
            selected_port: None,
            baud_rate: 115_200,
            is_open: false,
            serial_handle: None,
            serial_event_rx: None,
            output_lines: VecDeque::new(),
            output_scroll: 0,
            input_buffer: String::new(),
            focus: Focus::Ports,
        })
    }

    fn add_output_line<S: Into<String>>(&mut self, line: S) {
        self.output_lines.push_back(line.into());
        while self.output_lines.len() > MAX_OUTPUT_LINES {
            self.output_lines.pop_front();
        }
    }
}

pub fn run() -> Result<()> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let res = run_inner(&mut terminal);

    disable_raw_mode().ok();
    execute!(
        io::stdout(),
        terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    )
    .ok();
    terminal.show_cursor().ok();

    res
}

fn run_inner(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = AppState::new()?;

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key_event(&mut app, key)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            drain_serial_events(&mut app)?;
            last_tick = Instant::now();
        }
    }
    Ok(())
}

fn drain_serial_events(app: &mut AppState) -> Result<()> {
    let mut drained: Vec<SerialEvent> = Vec::new();
    if let Some(rx) = app.serial_event_rx.as_ref() {
        loop {
            match rx.try_recv() {
                Ok(ev) => drained.push(ev),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    for ev in drained {
        match ev {
            SerialEvent::Opened => {
                app.is_open = true;
                app.add_output_line("[opened]");
            }
            SerialEvent::Data(bytes) => {
                if let Ok(s) = String::from_utf8(bytes) {
                    for line in s.split_inclusive(['\n', '\r']).collect::<Vec<_>>() {
                        app.add_output_line(line.to_string());
                    }
                } else {
                    app.add_output_line("[binary data]");
                }
            }
            SerialEvent::Error(err) => {
                app.add_output_line(format!("[error] {err}"));
            }
            SerialEvent::Closed => {
                app.is_open = false;
                app.add_output_line("[closed]");
                app.serial_handle = None;
                app.serial_event_rx = None;
            }
        }
    }
    Ok(())
}

fn handle_key_event(app: &mut AppState, key: KeyEvent) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Ok(true);
    }
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Ports => Focus::Output,
                Focus::Output => Focus::Input,
                Focus::Input => Focus::Ports,
            };
        }
        KeyCode::BackTab => {
            app.focus = match app.focus {
                Focus::Ports => Focus::Input,
                Focus::Output => Focus::Ports,
                Focus::Input => Focus::Output,
            };
        }
        KeyCode::Char('r') => {
            app.ports = serial::list_ports()?;
            if app.ports.is_empty() {
                app.selected_port = None;
            } else {
                app.selected_port = Some(0);
            }
        }
        KeyCode::Char('b') => {
            const BAUDS: &[u32] = &[9600, 19200, 38400, 57600, 115200, 230400];
            let idx = BAUDS.iter().position(|b| *b == app.baud_rate).unwrap_or(0);
            let next = (idx + 1) % BAUDS.len();
            app.baud_rate = BAUDS[next];
        }
        KeyCode::Char('B') => {
            const BAUDS: &[u32] = &[9600, 19200, 38400, 57600, 115200, 230400];
            let idx = BAUDS.iter().position(|b| *b == app.baud_rate).unwrap_or(0);
            let prev = (idx + BAUDS.len() - 1) % BAUDS.len();
            app.baud_rate = BAUDS[prev];
        }
        _ => {
            match app.focus {
                Focus::Ports => match key.code {
                    KeyCode::Up => move_selection(app, -1),
                    KeyCode::Down => move_selection(app, 1),
                    KeyCode::Enter => toggle_port(app)?,
                    _ => {}
                },
                Focus::Output => match key.code {
                    KeyCode::PageUp => {
                        app.output_scroll = app.output_scroll.saturating_add(5);
                    }
                    KeyCode::PageDown => {
                        app.output_scroll = app.output_scroll.saturating_sub(5);
                    }
                    KeyCode::Home => {
                        app.output_scroll = app.output_lines.len() as u16;
                    }
                    KeyCode::End => {
                        app.output_scroll = 0;
                    }
                    _ => {}
                },
                Focus::Input => match key.code {
                    KeyCode::Enter => {
                        send_input(app)?;
                    }
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        app.input_buffer.push(c);
                    }
                    KeyCode::Tab => {}
                    _ => {}
                },
            }
        }
    }
    Ok(false)
}

fn move_selection(app: &mut AppState, delta: isize) {
    if app.ports.is_empty() {
        app.selected_port = None;
        return;
    }
    let len = app.ports.len() as isize;
    let current = app.selected_port.map(|i| i as isize).unwrap_or(0);
    let mut next = current + delta;
    if next < 0 {
        next = 0;
    }
    if next >= len {
        next = len - 1;
    }
    app.selected_port = Some(next as usize);
}

fn toggle_port(app: &mut AppState) -> Result<()> {
    if app.is_open {
        if let Some(handle) = app.serial_handle.take() {
            handle.close()?;
        }
        app.is_open = false;
        app.serial_event_rx = None;
        app.add_output_line("[closing...]");
        return Ok(());
    }

    let idx = app
        .selected_port
        .ok_or_else(|| anyhow!("no port selected"))?;
    let port = app
        .ports
        .get(idx)
        .ok_or_else(|| anyhow!("invalid port index"))?;
    let (handle, rx) = serial::open_port(&port.port_name, app.baud_rate)?;
    app.serial_handle = Some(handle);
    app.serial_event_rx = Some(rx);
    Ok(())
}

fn send_input(app: &mut AppState) -> Result<()> {
    if app.input_buffer.is_empty() {
        return Ok(());
    }
    if let Some(handle) = &app.serial_handle {
        let mut data = app.input_buffer.clone().into_bytes();
        data.push(b'\n');
        handle.write(data)?;
        app.add_output_line(format!(">> {}", app.input_buffer));
        app.input_buffer.clear();
    } else {
        app.add_output_line("[not open]");
    }
    Ok(())
}


