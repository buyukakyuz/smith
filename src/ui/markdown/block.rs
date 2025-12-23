use markdown::mdast::{Code, Heading, List, ListItem, Node, Paragraph};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use textwrap::wrap;

use crate::ui::theme::{BoxChars, Theme};

use super::context::RenderContext;
use super::inline::{collect_inline_spans, collect_text_from_nodes};

pub fn render_node(node: &Node, ctx: RenderContext) -> Vec<Line<'static>> {
    match node {
        Node::Root(root) => render_children(&root.children, ctx),

        Node::Heading(heading) => with_trailing_blank(render_heading(heading, ctx)),

        Node::Paragraph(para) => with_trailing_blank(render_paragraph(para, ctx)),

        Node::List(list) => with_trailing_blank(render_list(list, ctx)),

        Node::ListItem(item) => render_list_item(item, ctx),

        Node::Code(code) => with_trailing_blank(render_code_block(code, ctx)),

        Node::Break(_) | Node::ThematicBreak(_) => vec![Line::default()],

        Node::Blockquote(quote) => render_children(&quote.children, ctx.nested()),

        _ => Vec::new(),
    }
}

fn render_children(children: &[Node], ctx: RenderContext) -> Vec<Line<'static>> {
    children
        .iter()
        .flat_map(|child| render_node(child, ctx))
        .collect()
}

fn with_trailing_blank(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines.push(Line::default());
    lines
}

fn render_heading(heading: &Heading, ctx: RenderContext) -> Vec<Line<'static>> {
    let text = collect_text_from_nodes(&heading.children);

    vec![Line::from(vec![
        Span::raw(ctx.indent()),
        Span::styled(text, Theme::primary().add_modifier(Modifier::BOLD)),
    ])]
}

fn render_paragraph(para: &Paragraph, ctx: RenderContext) -> Vec<Line<'static>> {
    let spans = collect_inline_spans(&para.children);
    let full_text: String = spans.iter().map(|s| s.content.as_ref()).collect();

    wrap_text_to_lines(&full_text, ctx)
}

fn render_list(list: &List, ctx: RenderContext) -> Vec<Line<'static>> {
    render_children(&list.children, ctx)
}

fn render_list_item(item: &ListItem, ctx: RenderContext) -> Vec<Line<'static>> {
    let bullet = format!("{} ", BoxChars::DOT);
    let bullet_width = bullet.len();
    let indent = ctx.indent();

    let text_content = extract_paragraph_text(&item.children);

    let available_width = ctx
        .width
        .unwrap_or(80)
        .saturating_sub(ctx.indent_width() + bullet_width);

    let mut lines = render_bulleted_text(&text_content, &indent, &bullet, available_width, ctx);

    lines.extend(
        item.children
            .iter()
            .filter_map(|child| match child {
                Node::List(nested) => Some(render_list(nested, ctx.nested())),
                _ => None,
            })
            .flatten(),
    );

    lines
}

fn extract_paragraph_text(children: &[Node]) -> String {
    children
        .iter()
        .find_map(|child| match child {
            Node::Paragraph(para) => Some(collect_text_from_nodes(&para.children)),
            _ => None,
        })
        .unwrap_or_default()
}

fn render_bulleted_text(
    text: &str,
    indent: &str,
    bullet: &str,
    available_width: usize,
    ctx: RenderContext,
) -> Vec<Line<'static>> {
    if available_width > 0 && text.len() > available_width {
        wrap(text, available_width)
            .into_iter()
            .enumerate()
            .map(|(i, line_text)| {
                if i == 0 {
                    Line::from(vec![
                        Span::raw(indent.to_owned()),
                        Span::styled(bullet.to_owned(), Theme::primary()),
                        Span::raw(line_text.to_string()),
                    ])
                } else {
                    let continuation = " ".repeat(ctx.indent_width() + bullet.len());
                    Line::from(vec![
                        Span::raw(continuation),
                        Span::raw(line_text.to_string()),
                    ])
                }
            })
            .collect()
    } else {
        vec![Line::from(vec![
            Span::raw(indent.to_owned()),
            Span::styled(bullet.to_owned(), Theme::primary()),
            Span::raw(text.to_owned()),
        ])]
    }
}

fn render_code_block(code: &Code, ctx: RenderContext) -> Vec<Line<'static>> {
    let indent = ctx.indent();
    let lang = code.lang.as_deref().unwrap_or("code");

    let mut lines = Vec::with_capacity(code.value.lines().count() + 2);

    lines.push(Line::from(vec![
        Span::raw(indent.clone()),
        Span::styled(
            format!("{} {lang}", BoxChars::ROUND_TOP_LEFT),
            Theme::border(),
        ),
    ]));

    for line in code.value.lines() {
        lines.push(Line::from(vec![
            Span::raw(indent.clone()),
            Span::styled(format!("{} {line}", BoxChars::VERTICAL), Theme::muted()),
        ]));
    }

    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(BoxChars::ROUND_BOTTOM_LEFT.to_string(), Theme::border()),
    ]));

    lines
}

fn wrap_text_to_lines(text: &str, ctx: RenderContext) -> Vec<Line<'static>> {
    let indent = ctx.indent();
    let available = ctx.available_width();

    if available == 0 {
        return Vec::new();
    }

    text.lines()
        .flat_map(|line| {
            if line.len() <= available || ctx.width.is_none() {
                vec![Line::from(vec![
                    Span::raw(indent.clone()),
                    Span::styled(line.to_string(), Style::default()),
                ])]
            } else {
                wrap(line, available)
                    .into_iter()
                    .map(|wrapped| {
                        Line::from(vec![
                            Span::raw(indent.clone()),
                            Span::styled(wrapped.to_string(), Style::default()),
                        ])
                    })
                    .collect()
            }
        })
        .collect()
}
