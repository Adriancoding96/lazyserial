use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::*;
use unicode_width::UnicodeWidthStr;

use crate::app::{AppState, Focus};

pub fn draw(frame: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)].as_ref(),
        )
        .split(frame.size());

    draw_header(frame, chunks[0], app);
    draw_body(frame, chunks[1], app);
    draw_footer(frame, chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &AppState) {
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(" setial-tui ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)));
    spans.push(Span::raw("  q:quit  TAB:focus  r:refresh  b/B:baud  Enter:open/close "));
    spans.push(Span::styled(
        format!(" [baud:{}] ", app.baud_rate),
        Style::default().fg(Color::Yellow),
    ));
    if let Some(idx) = app.selected_port {
        spans.push(Span::styled(
            format!(" port:{} ", app.ports[idx].port_name),
            Style::default().fg(Color::Green),
        ));
    }
    spans.push(Span::styled(
        if app.is_open { " OPEN " } else { " CLOSED " },
        if app.is_open {
            Style::default().fg(Color::Black).bg(Color::Green)
        } else {
            Style::default().fg(Color::Black).bg(Color::Red)
        },
    ));

    let block = Block::default().borders(Borders::ALL).title("Help");
    let p = Paragraph::new(Text::from(Line::from(spans))).block(block);
    frame.render_widget(p, area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);

    draw_ports(frame, chunks[0], app);
    draw_output(frame, chunks[1], app);
}

fn draw_ports(frame: &mut Frame, area: Rect, app: &AppState) {
    let items: Vec<ListItem> = app
        .ports
        .iter()
        .map(|p| {
            let mut line = vec![
                Span::styled(&p.port_name, Style::default().fg(Color::White)),
            ];
            if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
                let tail = format!(
                    "  {} {} {}",
                    info.manufacturer.clone().unwrap_or_default(),
                    info.product.clone().unwrap_or_default(),
                    info.serial_number.clone().unwrap_or_default()
                );
                line.push(Span::styled(tail, Style::default().fg(Color::DarkGray)));
            }
            ListItem::new(Line::from(line))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Ports"))
        .highlight_style(Style::default().bg(Color::Blue))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(app.selected_port);
    frame.render_stateful_widget(list, area, &mut state);

    if app.focus == Focus::Ports {
        frame.set_cursor(area.x + 1, area.y + 1);
    }
}

fn draw_output(frame: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default().borders(Borders::ALL).title("Output");

    let height = area.height.saturating_sub(2) as usize; // borders
    let total = app.output_lines.len();
    let scroll_back = app.output_scroll as usize;
    let start = total.saturating_sub(height + scroll_back);
    let end = total.saturating_sub(scroll_back);
    let visible = app.output_lines.iter().skip(start).take(end - start);

    let text: Vec<Line> = visible
        .map(|l| Line::from(Span::raw(l.clone())))
        .collect();
    let p = Paragraph::new(Text::from(text))
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &AppState) {
    let title = match app.focus {
        Focus::Ports => "Ports",
        Focus::Output => "Output",
        Focus::Input => "Input",
    };
    let block = Block::default().borders(Borders::ALL).title(title);

    let style = if app.focus == Focus::Input {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let p = Paragraph::new(app.input_buffer.as_str()).style(style).block(block);
    frame.render_widget(p, area);

    if app.focus == Focus::Input {
        let x = area.x + 1 + app.input_buffer.width() as u16;
        let y = area.y + 1;
        frame.set_cursor(x, y);
    }
}


