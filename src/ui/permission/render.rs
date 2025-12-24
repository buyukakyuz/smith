use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::constants::{MAX_DIALOG_WIDTH, MAX_PREVIEW_LINES, MIN_DIALOG_WIDTH, OPTION_WIDTH};
use super::input::{Selection, get_action_verb};
use crate::permission::types::{PermissionRequest, PermissionType};
use crate::ui::theme::{BoxChars, BrandColors};

pub struct PromptRenderer<'a> {
    request: &'a PermissionRequest,
    selected: Selection,
    content_preview: Option<&'a str>,
}

impl<'a> PromptRenderer<'a> {
    pub const fn new(
        request: &'a PermissionRequest,
        selected: Selection,
        content_preview: Option<&'a str>,
    ) -> Self {
        Self {
            request,
            selected,
            content_preview,
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let dialog_width = self.calculate_dialog_width(area.width);

        let dialog_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(dialog_width), Constraint::Min(0)])
            .split(area)[0];

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(5),
                Constraint::Length(1),
            ])
            .split(dialog_area);

        Self::draw_horizontal_border(frame, main_chunks[0]);
        self.draw_content(frame, main_chunks[1]);
        Self::draw_horizontal_border(frame, main_chunks[2]);
    }

    fn draw_content(&self, frame: &mut Frame, area: Rect) {
        let constraints = if self.content_preview.is_some() {
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
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut chunk_iter = chunks.iter();

        if let Some(&header_area) = chunk_iter.next() {
            self.draw_action_header(frame, header_area);
        }

        if let Some(preview) = self.content_preview {
            if let Some(&divider_area) = chunk_iter.next() {
                Self::draw_divider(frame, divider_area);
            }
            if let Some(&preview_area) = chunk_iter.next() {
                Self::draw_preview(frame, preview_area, preview);
            }
        }

        if let Some(&divider_area) = chunk_iter.next() {
            Self::draw_divider(frame, divider_area);
        }
        if let Some(&options_area) = chunk_iter.next() {
            self.draw_options(frame, options_area);
        }
    }

    fn calculate_dialog_width(&self, terminal_width: u16) -> u16 {
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);

        let target_width = to_u16(self.request.target.len()) + 30;
        let context_width = self
            .request
            .context
            .as_ref()
            .map_or(0, |c| to_u16(c.len()) + 10);
        let preview_width = self.content_preview.map_or(0, |content| {
            let max_line = content
                .lines()
                .take(MAX_PREVIEW_LINES)
                .map(str::len)
                .max()
                .unwrap_or(0);
            to_u16(max_line) + 10
        });

        [
            MIN_DIALOG_WIDTH,
            target_width,
            context_width,
            preview_width,
            OPTION_WIDTH,
        ]
        .into_iter()
        .max()
        .unwrap_or(MIN_DIALOG_WIDTH)
        .clamp(MIN_DIALOG_WIDTH, MAX_DIALOG_WIDTH)
        .min(terminal_width.saturating_sub(4))
    }

    fn draw_horizontal_border(frame: &mut Frame, area: Rect) {
        let border = BoxChars::HORIZONTAL.repeat(area.width as usize);
        frame.render_widget(Paragraph::new(border).fg(BrandColors::DARK_GRAY), area);
    }

    fn draw_divider(frame: &mut Frame, area: Rect) {
        let divider = BoxChars::DIVIDER_LIGHT.repeat(area.width as usize);
        frame.render_widget(Paragraph::new(divider).fg(BrandColors::DARK_GRAY), area);
    }

    fn draw_action_header(&self, frame: &mut Frame, area: Rect) {
        let (action_text, action_color) = self.action_display();

        let mut lines = vec![
            Line::default(),
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

        frame.render_widget(Paragraph::new(lines), area);
    }

    const fn action_display(&self) -> (&'static str, ratatui::style::Color) {
        match self.request.operation_type {
            PermissionType::FileWrite => {
                let text = if self.content_preview.is_some() {
                    "Create file"
                } else {
                    "Write to file"
                };
                (text, BrandColors::CYAN)
            }
            PermissionType::FileRead => ("Read file", BrandColors::CYAN),
            PermissionType::FileDelete => ("Delete file", BrandColors::RED),
            PermissionType::CommandExecute => ("Execute command", BrandColors::AMBER),
            PermissionType::NetworkAccess => ("Network request to", BrandColors::AMBER),
            PermissionType::SystemModification => ("System modification", BrandColors::RED),
        }
    }

    fn draw_preview(frame: &mut Frame, area: Rect, content: &str) {
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

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }

    fn draw_options(&self, frame: &mut Frame, area: Rect) {
        let question = Line::from(vec![
            Span::raw(" Do you want to "),
            Span::raw(get_action_verb(self.request.operation_type)).bold(),
            Span::raw(" "),
            Span::raw(&self.request.target).fg(BrandColors::CYAN),
            Span::raw("?"),
        ]);

        let options = [
            (Selection::AllowOnce, "Yes"),
            (
                Selection::AllowSession,
                "Yes, allow all edits during this session (shift+tab)",
            ),
            (
                Selection::Feedback,
                "Type here to tell Claude what to do differently",
            ),
        ];

        let option_lines: Vec<Line> = options
            .into_iter()
            .map(|(sel, text)| self.format_option(sel, text))
            .collect();

        let mut lines = vec![question];
        lines.extend(option_lines);

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn format_option(&self, selection: Selection, text: &str) -> Line<'static> {
        let is_selected = selection == self.selected;
        let index = selection.index() + 1;

        let (indicator, number, option_text) = if is_selected {
            (
                Span::raw(format!(" {} ", BoxChars::ARROW_RIGHT))
                    .fg(BrandColors::CYAN)
                    .bold(),
                Span::raw(format!("{index}. ")).fg(BrandColors::CYAN).bold(),
                Span::raw(text.to_owned()).fg(BrandColors::WHITE).bold(),
            )
        } else {
            (
                Span::raw("   "),
                Span::raw(format!("{index}. ")).fg(BrandColors::GRAY),
                Span::raw(text.to_owned()).fg(BrandColors::GRAY),
            )
        };

        Line::from(vec![indicator, number, option_text])
    }
}
