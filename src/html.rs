pub fn html_to_text(html: &str) -> String {
    // Use the lightweight `tl` crate to parse HTML.
    // If parsing fails, return the original string.
    let dom = match tl::parse(html, tl::ParserOptions::default()) {
        Ok(dom) => dom,
        Err(_) => return html.to_string(),
    };

    let parser = dom.parser();
    let mut out = String::new();

    // Walk the first node in the document and gather text
    if let Some(root) = dom.nodes().iter().next().cloned() {
        extract_text(root, &dom, parser, &mut out);
    }

    out
}

fn extract_text(handle: tl::NodeHandle, dom: &tl::VDom, parser: &tl::Parser, out: &mut String) {
    if let Some(node) = dom.get(handle) {
        match node {
            tl::Node::Tag(tag) => {
                for child in tag.children().iter() {
                    extract_text(*child, dom, parser, out);
                }
                if let Some(name) = tag.name().as_ref().and_then(|n| n.as_utf8_str()) {
                    if name == "br" || name == "p" {
                        out.push('\n');
                    }
                }
            }
            tl::Node::Raw(text) => out.push_str(text.as_utf8_str()),
            _ => {}
        }
    }
}
