pub fn html_to_text(html: &str) -> String {
    // Use the lightweight `tl` crate to parse HTML.
    // If parsing fails, return the original string.
    let dom = match tl::parse(html, tl::ParserOptions::default()) {
        Ok(dom) => dom,
        Err(_) => return html.to_string(),
    };

    let parser = dom.parser();
    let mut out = String::new();
    for child in dom.children_of(dom.document()) {
        extract_text(child, &dom, parser, &mut out);
    }
    out
}

fn extract_text(handle: tl::NodeHandle, dom: &tl::VDom, parser: &tl::Parser, out: &mut String) {
    let node = &dom[handle];
    match node {
        tl::Node::Tag(tag) => {
            for child in tag.children() {
                extract_text(*child, dom, parser, out);
            }
            if let Some(name) = tag.name().as_utf8_str() {
                if name == "br" || name == "p" {
                    out.push('\n');
                }
            }
        }
        tl::Node::Text(text) => out.push_str(text.as_utf8_str()),
        _ => {}
    }
}
