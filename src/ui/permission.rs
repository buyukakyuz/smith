use super::theme::{BoxChars, BrandColors};
use crate::permission::types::{PermissionRequest, PermissionResponse, PermissionType};
use crate::tui::app::TerminalGuard;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use std::io::{self, Write, stdout};

const MAX_PREVIEW_LINES: usize = 20;
const MIN_DIALOG_WIDTH: u16 = 60;
const MAX_DIALOG_WIDTH: u16 = 120;

pub struct PermissionPrompt {
    request: PermissionRequest,
    selected: usize,
    content_preview: Option<String>,
}

impl PermissionPrompt {
    #[must_use]
    pub const fn new(request: PermissionRequest) -> Self {
        Self {
            request,
            selected: 0,
            content_preview: None,
        }
    }

    pub fn run(&mut self) -> io::Result<PermissionResponse> {
        let _guard = TerminalGuard::acquire()?;

        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        self.run_ui(&mut terminal)
    }

    fn run_ui(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<PermissionResponse> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if let Event::Key(key) = event::read()?
                && let Some(response) = self.handle_key(key)
            {
                return Ok(response);
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let dialog_width = self.calculate_dialog_width(area.width);

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(dialog_width), Constraint::Min(0)])
            .split(area);

        let dialog_area = horizontal_chunks[0];

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(1),
            ])
            .split(dialog_area);

        Self::draw_top_border(frame, main_chunks[0]);

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if self.content_preview.is_some() {
                vec![
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Min(10),
                    Constraint::Length(1),
                    Constraint::Length(5),
                ]
            } else {
                vec![
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Min(5),
                ]
            })
            .split(main_chunks[1]);

        let mut idx = 0;

        self.draw_action_header(frame, content_chunks[idx]);
        idx += 1;

        if self.content_preview.is_some() {
            Self::draw_divider(frame, content_chunks[idx]);
            idx += 1;
            self.draw_preview(frame, content_chunks[idx]);
            idx += 1;
        }

        Self::draw_divider(frame, content_chunks[idx]);
        idx += 1;

        self.draw_options(frame, content_chunks[idx]);

        Self::draw_bottom_border(frame, main_chunks[2]);
    }

    fn calculate_dialog_width(&self, terminal_width: u16) -> u16 {
        let mut max_width = MIN_DIALOG_WIDTH;

        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);

        let target_len = to_u16(self.request.target.len()) + 30;
        max_width = max_width.max(target_len);

        if let Some(context) = &self.request.context {
            let context_len = to_u16(context.len()) + 10;
            max_width = max_width.max(context_len);
        }

        if let Some(content) = &self.content_preview {
            let max_line_len = to_u16(
                content
                    .lines()
                    .take(MAX_PREVIEW_LINES)
                    .map(str::len)
                    .max()
                    .unwrap_or(0),
            );

            max_width = max_width.max(max_line_len + 10);
        }

        let option_width = 70;
        max_width = max_width.max(option_width);

        max_width
            .clamp(MIN_DIALOG_WIDTH, MAX_DIALOG_WIDTH)
            .min(terminal_width.saturating_sub(4))
    }

    fn draw_top_border(frame: &mut Frame, area: Rect) {
        let border = BoxChars::HORIZONTAL.repeat(area.width as usize);
        let text = Paragraph::new(border).fg(BrandColors::DARK_GRAY);
        frame.render_widget(text, area);
    }

    fn draw_bottom_border(frame: &mut Frame, area: Rect) {
        let border = BoxChars::HORIZONTAL.repeat(area.width as usize);
        let text = Paragraph::new(border).fg(BrandColors::DARK_GRAY);
        frame.render_widget(text, area);
    }

    fn draw_divider(frame: &mut Frame, area: Rect) {
        let divider = BoxChars::DIVIDER_LIGHT.repeat(area.width as usize);
        let text = Paragraph::new(divider).fg(BrandColors::DARK_GRAY);
        frame.render_widget(text, area);
    }

    fn draw_action_header(&self, frame: &mut Frame, area: Rect) {
        let (action_text, action_color) = match self.request.operation_type {
            PermissionType::FileWrite => {
                if self.content_preview.is_some() {
                    ("Create file", BrandColors::CYAN)
                } else {
                    ("Write to file", BrandColors::CYAN)
                }
            }
            PermissionType::FileRead => ("Read file", BrandColors::CYAN),
            PermissionType::FileDelete => ("Delete file", BrandColors::RED),
            PermissionType::CommandExecute => ("Execute command", BrandColors::AMBER),
            PermissionType::NetworkAccess => ("Network request to", BrandColors::AMBER),
            PermissionType::SystemModification => ("System modification", BrandColors::RED),
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw(" "),
                Span::raw(action_text).fg(action_color).bold(),
                Span::raw(" "),
                Span::raw(&self.request.target).fg(BrandColors::WHITE),
            ]),
        ];

        if let Some(context) = &self.request.context {
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::raw(context).fg(BrandColors::GRAY),
            ]));
        }

        let header = Paragraph::new(lines);
        frame.render_widget(header, area);
    }

    fn draw_preview(&self, frame: &mut Frame, area: Rect) {
        if let Some(content) = &self.content_preview {
            let lines: Vec<Line> = content
                .lines()
                .take(MAX_PREVIEW_LINES)
                .enumerate()
                .map(|(i, line)| {
                    Line::from(vec![
                        Span::raw(format!("   {:3} ", i + 1)).fg(BrandColors::GRAY),
                        Span::raw(line),
                    ])
                })
                .collect();

            let preview = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(preview, area);
        }
    }

    fn draw_options(&self, frame: &mut Frame, area: Rect) {
        let question = Line::from(vec![
            Span::raw(" Do you want to "),
            Span::raw(self.get_action_verb()).bold(),
            Span::raw(" "),
            Span::raw(&self.request.target).fg(BrandColors::CYAN),
            Span::raw("?"),
        ]);

        let opt1 = self.format_option(0, "Yes");
        let opt2 = self.format_option(1, "Yes, allow all edits during this session (shift+tab)");
        let opt3 = self.format_option(2, "Type here to tell Claude what to do differently");

        let widget = Paragraph::new(vec![question, opt1, opt2, opt3]);

        frame.render_widget(widget, area);
    }

    const fn get_action_verb(&self) -> &str {
        match self.request.operation_type {
            PermissionType::FileWrite => "create",
            PermissionType::FileRead => "read",
            PermissionType::FileDelete => "delete",
            PermissionType::CommandExecute => "execute",
            PermissionType::NetworkAccess => "connect to",
            PermissionType::SystemModification => "modify",
        }
    }

    fn format_option<'a>(&self, index: usize, text: &'a str) -> Line<'a> {
        let is_selected = index == self.selected;

        let indicator = if is_selected {
            Span::raw(format!(" {} ", BoxChars::ARROW_RIGHT))
                .fg(BrandColors::CYAN)
                .bold()
        } else {
            Span::raw("   ")
        };

        let number = if is_selected {
            Span::raw(format!("{}. ", index + 1))
                .fg(BrandColors::CYAN)
                .bold()
        } else {
            Span::raw(format!("{}. ", index + 1)).fg(BrandColors::GRAY)
        };

        let option_text = if is_selected {
            Span::raw(text).fg(BrandColors::WHITE).bold()
        } else {
            Span::raw(text).fg(BrandColors::GRAY)
        };

        Line::from(vec![indicator, number, option_text])
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<PermissionResponse> {
        match (key.code, key.modifiers) {
            (KeyCode::Up | KeyCode::Char('k'), _) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            (KeyCode::Down | KeyCode::Char('j'), _) => {
                if self.selected < 2 {
                    self.selected += 1;
                }
                None
            }
            (KeyCode::Char('1'), _) => Some(PermissionResponse::AllowOnce),
            (KeyCode::Char('2'), _) | (KeyCode::Tab, KeyModifiers::SHIFT) => {
                Some(PermissionResponse::AllowSession)
            }
            (KeyCode::Char('3'), _) => Some(Self::get_feedback_input()),
            (KeyCode::Enter, _) => Some(self.get_response()),
            (KeyCode::Esc, _) => Some(PermissionResponse::TellModelDifferently(
                "User cancelled this operation. Please ask what to do instead.".to_string(),
            )),
            _ => None,
        }
    }

    fn get_response(&self) -> PermissionResponse {
        match self.selected {
            0 => PermissionResponse::AllowOnce,
            1 => PermissionResponse::AllowSession,
            2 => Self::get_feedback_input(),
            _ => PermissionResponse::TellModelDifferently(
                "User cancelled this operation. Please ask what to do instead.".to_string(),
            ),
        }
    }

    fn get_feedback_input() -> PermissionResponse {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);

        println!("\nTell the model what to do instead:");
        print!("> ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let feedback = input.trim().to_string();
            if feedback.is_empty() {
                PermissionResponse::TellModelDifferently(
                    "User cancelled this operation. Please ask what to do instead.".to_string(),
                )
            } else {
                PermissionResponse::TellModelDifferently(feedback)
            }
        } else {
            PermissionResponse::TellModelDifferently(
                "User cancelled this operation. Please ask what to do instead.".to_string(),
            )
        }
    }
}

pub fn prompt_user(request: &PermissionRequest) -> io::Result<PermissionResponse> {
    let mut prompt = PermissionPrompt::new(request.clone());
    prompt.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_prompt_creation() {
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let prompt = PermissionPrompt::new(request);
        assert_eq!(prompt.selected, 0);
    }
}
