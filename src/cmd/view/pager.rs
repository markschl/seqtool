use std::io;
use std::time::{Duration, Instant};

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Layout, Size},
    style::Stylize,
    symbols::{block, line, scrollbar},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Ok,
    MissingLines,
    Quit,
}

#[derive(Debug)]
pub struct GrowingPager {
    last_tick: Instant,
    status: Status,
    current_size: Size,
    color_scale: Option<Vec<Span<'static>>>,
    lines: Vec<Line<'static>>,
    line_lengths: Vec<usize>,
    v_state: ScrollbarState,
    h_state: ScrollbarState,
    v_scroll: usize,
    h_scroll: usize,
}

impl GrowingPager {
    const TICK_RATE: Duration = Duration::from_millis(200);

    pub fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            status: Status::MissingLines,
            current_size: Size::default(),
            color_scale: None,
            lines: Vec::new(),
            line_lengths: Vec::new(),
            v_state: ScrollbarState::default(),
            h_state: ScrollbarState::default(),
            v_scroll: 0,
            h_scroll: 0,
        }
    }

    pub fn set_color_scale(&mut self, color_scale: Option<Vec<Span<'static>>>) {
        self.color_scale = color_scale;
    }

    pub fn check_draw(
        &mut self,
        terminal: &mut DefaultTerminal,
        draw_incomplete: bool,
    ) -> io::Result<Status> {
        let mut timeout = if self.status == Status::MissingLines {
            Duration::ZERO
        } else {
            Self::TICK_RATE.saturating_sub(self.last_tick.elapsed())
        };
        let mut changed = self.status == Status::MissingLines;
        while event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    use KeyCode::*;
                    match key.code {
                        Char('q') | Char('Q') | Esc => return Ok(Status::Quit),
                        Down | PageDown => {
                            if self.v_scroll < self.lines.len() {
                                self.v_scroll =
                                    self.v_scroll.saturating_add(if key.code == PageDown {
                                        self.current_size.height as usize
                                    } else {
                                        1
                                    });
                                self.v_state = self.v_state.position(self.v_scroll);
                                changed = true;
                            }
                        }
                        Up | PageUp | Home => {
                            if self.v_scroll > 0 {
                                self.v_scroll = match key.code {
                                    Up => self.v_scroll - 1,
                                    PageUp => self
                                        .v_scroll
                                        .saturating_sub(self.current_size.height as usize),
                                    Home => 0,
                                    _ => unreachable!(),
                                };
                                self.v_state = self.v_state.position(self.v_scroll);
                                changed = true;
                            }
                        }
                        Left => {
                            if self.h_scroll > 0 {
                                let delta = (self.current_size.width / 4).max(1);
                                self.h_scroll = self.h_scroll.saturating_sub(delta as usize);
                                self.h_state = self.h_state.position(self.h_scroll);
                                changed = true;
                            }
                        }
                        Right => {
                            let delta = (self.current_size.width / 4).max(1);
                            self.h_scroll = self.h_scroll.saturating_add(delta as usize);
                            self.h_state = self.h_state.position(self.h_scroll);
                            changed = true;
                        }
                        _ => {}
                    }
                }
                Event::Resize(w, h) => {
                    self.current_size.width = w;
                    self.current_size.height = h;
                    changed = true;
                }
                _ => {}
            }
            timeout = Duration::ZERO;
        }
        self.last_tick = Instant::now();
        if changed {
            self.current_size = terminal.size()?;
            if self.lines.len() < self.v_scroll + (self.current_size.height as usize)
                && !draw_incomplete
            {
                self.status = Status::MissingLines;
            } else {
                self.status = Status::Ok;
                terminal.draw(|frame| self.draw(frame))?;
            }
        }
        Ok(self.status)
    }

    pub fn add(&mut self, line: Line<'static>, len: usize) {
        self.lines.push(line);
        self.line_lengths.push(len);
        self.v_state = self.v_state.content_length(self.lines.len());
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        // layout
        let area = frame.area();
        let h_chunks = Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(area);
        let v_chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(h_chunks[0]);

        let start: usize = self.v_scroll;
        let end = (self.v_scroll + v_chunks[1].height as usize).min(self.lines.len());

        // title
        let mut title = Line::from(
            format!(
                " ► Records {} - {} | Press 'Esc' or 'q' to quit |",
                (start + 1).min(end),
                end,
            )
            .bold(),
        );
        if let Some(spans) = self.color_scale.as_ref() {
            title.push_span(Span::raw(" Qual: "));
            for span in spans {
                title.push_span(span.clone());
            }
        }
        frame.render_widget(title, v_chunks[0]);

        // sequences
        let paragraph =
            Paragraph::new(self.lines[start..end].to_vec()).scroll((0, self.h_scroll as u16));
        frame.render_widget(paragraph, v_chunks[1]);

        // scroll bars
        let max_len = self.line_lengths[start..end]
            .iter()
            .max()
            .copied()
            .unwrap_or(0);
        self.h_state = self.h_state.content_length(max_len);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .symbols(scrollbar::VERTICAL)
                .thumb_symbol(block::FULL)
                .track_symbol(Some(line::VERTICAL))
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            h_chunks[1],
            &mut self.v_state,
        );
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
                .thumb_symbol("🬋")
                .track_symbol(Some(line::HORIZONTAL))
                .begin_symbol(Some("◄"))
                .end_symbol(Some("►")),
            v_chunks[2],
            &mut self.h_state,
        );
    }
}
