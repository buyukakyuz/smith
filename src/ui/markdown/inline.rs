use std::borrow::Cow;

use markdown::mdast::Node;
use ratatui::{style::Modifier, text::Span};

use crate::ui::theme::Theme;

pub fn collect_text_from_nodes(nodes: &[Node]) -> String {
    nodes.iter().map(collect_text).collect()
}

pub fn collect_text(node: &Node) -> Cow<'static, str> {
    match node {
        Node::Text(text) => Cow::Owned(text.value.clone()),
        Node::InlineCode(code) => Cow::Owned(code.value.clone()),
        Node::Strong(strong) => Cow::Owned(collect_text_from_nodes(&strong.children)),
        Node::Emphasis(em) => Cow::Owned(collect_text_from_nodes(&em.children)),
        Node::Link(link) => Cow::Owned(collect_text_from_nodes(&link.children)),
        _ => Cow::Borrowed(""),
    }
}

pub fn collect_inline_spans(nodes: &[Node]) -> Vec<Span<'static>> {
    nodes.iter().flat_map(render_inline).collect()
}

fn render_inline(node: &Node) -> Vec<Span<'static>> {
    match node {
        Node::Text(text) => vec![Span::raw(text.value.clone())],

        Node::Strong(strong) => {
            let text = collect_text_from_nodes(&strong.children);
            vec![Span::styled(text, Modifier::BOLD)]
        }

        Node::Emphasis(em) => collect_inline_spans(&em.children),

        Node::InlineCode(code) => {
            vec![Span::styled(code.value.clone(), Theme::secondary())]
        }

        Node::Link(link) => collect_inline_spans(&link.children),

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markdown::mdast::Text;

    #[test]
    fn test_collect_text_from_text_node() {
        let node = Node::Text(Text {
            value: "hello".into(),
            position: None,
        });
        assert_eq!(collect_text(&node).as_ref(), "hello");
    }

    #[test]
    fn test_collect_inline_spans_empty() {
        let spans = collect_inline_spans(&[]);
        assert!(spans.is_empty());
    }
}
