// src/deck/html_parser.rs

use tl::{parse, NodeHandle, ParserOptions};

#[derive(Debug, Clone, Default)]
pub struct TextSpan {
    pub text: String,
    pub new_text_block: bool,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_ruby_base: bool,
    pub ruby_text: Option<String>,
    pub is_newline: bool,
}

pub fn parse_html_to_spans(html: &str) -> Vec<TextSpan> {
    // try full-HTML parse, else plain text
    let dom = parse(html, ParserOptions::default())
        .unwrap_or_else(|_| parse(html, ParserOptions::new()).unwrap());
    let parser = dom.parser();

    // queue up the NodeHandles at the top level (or under <body>)
    let mut queue = Vec::<NodeHandle>::new();
    if let Some(mut sel) = dom.query_selector("body") {
        if let Some(body_h) = sel.next() {
            if let Some(tag) = body_h.get(parser).and_then(|n| n.as_tag()) {
                queue.extend(tag.children().top().to_vec());
            }
        }
    }
    if queue.is_empty() {
        queue.extend(dom.children().iter().cloned());
    }

    let mut spans = Vec::new();
    for h in queue {
        process_node(h, parser, &mut spans, TextSpan::default());
    }
    spans
}

fn process_node(
    handle: NodeHandle,
    parser: &tl::Parser<'_>,
    spans: &mut Vec<TextSpan>,
    fmt: TextSpan,
) {
    let node = handle.get(parser).unwrap();

    // If it's a tag (element), adjust formatting and recurse
    if let Some(tag) = node.as_tag() {
        let mut nf = fmt.clone();
        let mut is_br = false;

        match tag.name().as_utf8_str().as_ref() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                nf.is_bold = true; // Treat other headings as bold
                nf.new_text_block = true;
            } 
            "br" => {
                is_br = true;
                nf.new_text_block = true;
                spans.push(TextSpan {
                    text: "\n".into(),
                    ..fmt.clone()
                });
            }
            "b" => nf.is_bold = true,
            "i" => nf.is_italic = true,
            "hr/" => {
                nf.new_text_block = true;
                spans.push(TextSpan {
                    text: "\n".into(),
                    ..fmt.clone()
                });
            }
            "ruby" => {
                // This is a placeholder for now. The logic below will handle children.
            }
            "rb" => {
                nf.is_ruby_base = true; // Base text of the ruby annotation
            }
            "rt" => {
                // Ruby text (furigana)
                // For now, it's just processed as normal text within a span.
                // The parent `ruby` tag processing will fill the `ruby_text` field.
            }
            _ => {}
        }

        // Special handling for <ruby> to capture ruby_text and combine base text
        if tag.name().as_utf8_str().as_ref() == "ruby" {
            let mut base_text_spans = Vec::new();
            let mut ruby_text_content: Option<String> = None;

            for child in tag.children().top().to_vec() {
                let child_node = child.get(parser).unwrap();
                if let Some(child_tag) = child_node.as_tag() {
                    match child_tag.name().as_utf8_str().as_ref() {
                        "rb" => {
                            let mut rb_spans = Vec::new();
                            for rb_child in child_tag.children().top().to_vec() {
                                process_node(rb_child, parser, &mut rb_spans, nf.clone());
                            }
                            base_text_spans.extend(rb_spans);
                        }
                        "rt" => {
                            let mut rt_spans = Vec::new();
                            for rt_child in child_tag.children().top().to_vec() {
                                process_node(rt_child, parser, &mut rt_spans, nf.clone());
                            }
                            ruby_text_content = Some(rt_spans.into_iter().map(|s| s.text).collect());
                        }
                        _ => {}
                    }
                } else if let Some(child_bytes) = child_node.as_raw() {
                    base_text_spans.push(TextSpan { text: child_bytes.as_utf8_str().to_string(), ..nf.clone() });
                }
            }
            
            if !base_text_spans.is_empty() {
                let combined_base_text: String = base_text_spans.into_iter().map(|s| s.text).collect();
                spans.push(TextSpan {
                    text: combined_base_text,
                    is_bold: nf.is_bold,
                    is_italic: nf.is_italic,
                    is_ruby_base: true,
                    ruby_text: ruby_text_content,
                    new_text_block: false,
                    is_newline: false,
                });
            }
        } else if !is_br { // Normal element, process children recursively, if not a <br> or <ruby>
            for child in tag.children().top().to_vec() {
                process_node(child, parser, spans, nf.clone());
            }
        }

        // --- Add a newline after block-level elements for separation ---
        match tag.name().as_utf8_str().as_ref() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "hr" | "li" | "hr/" | "ul" => {
                // Ensure a single newline is added after these block elements.
                // Check if the last span is *not* already a newline.
                if spans.last().map_or(true, |s| s.text != "\n") { // Add if empty or last is not newline
                    spans.push(TextSpan { text: "\n".into(), ..fmt.clone() });
                }
            }
            _ => {}
        }
    }
    // Otherwise it's “raw” (text, comment, doctype). We only want real text.
    else if let Some(bytes) = node.as_raw() {
        // skip comment nodes entirely
        if node.as_comment().is_some() {
            return;
        }
        let txt = bytes.as_utf8_str().to_string();

        if !txt.trim().is_empty() {
            let mut span = fmt.clone();
            span.text = txt;
            spans.push(span);
        }
    }
}