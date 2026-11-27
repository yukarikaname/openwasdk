//! A small tag scanner for AppxManifest documents.
//!
//! It only understands the default namespace: a prefixed element name is kept
//! as written, not expanded, so `<x:Extension>` is a different name from
//! `Extension`. Comments and processing instructions are skipped; CDATA and a
//! DOCTYPE are rejected. Text and attribute values decode only the five XML
//! predefined entities, and any other entity is an error.
//!
//! [`validate`] requires matched tags and exactly one root element, so a
//! truncated document never produces a partial tree.

#[derive(Debug)]
pub(super) struct Tag {
    pub(super) name: String,
    pub(super) attributes: Vec<(String, String)>,
    pub(super) closing: bool,
    pub(super) self_closing: bool,
}

impl Tag {
    pub(super) fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug)]
pub(super) enum Node {
    Tag(Tag),
    Text(String),
}

pub(super) fn scan(xml: &str) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    let mut index = 0;
    while index < xml.len() {
        if xml[index..].starts_with("<!--") {
            let end = xml[index + 4..]
                .find("-->")
                .map(|offset| index + 4 + offset)
                .ok_or_else(|| format!("unterminated comment at byte {index}"))?;
            index = end + 3;
        } else if xml[index..].starts_with("<?") {
            let end = xml[index + 2..]
                .find("?>")
                .map(|offset| index + 2 + offset)
                .ok_or_else(|| format!("unterminated processing instruction at byte {index}"))?;
            index = end + 2;
        } else if xml[index..].starts_with("<!") {
            return Err(format!("unsupported declaration at byte {index}"));
        } else if xml[index..].starts_with("</") {
            let end = find_tag_end(xml, index + 2)?;
            nodes.push(Node::Tag(parse_close(&xml[index + 2..end], index)?));
            index = end + 1;
        } else if xml[index..].starts_with('<') {
            let end = find_tag_end(xml, index + 1)?;
            nodes.push(Node::Tag(parse_open(&xml[index + 1..end], index)?));
            index = end + 1;
        } else {
            let end = xml[index..]
                .find('<')
                .map(|offset| index + offset)
                .unwrap_or(xml.len());
            nodes.push(Node::Text(decode_entities(&xml[index..end])?));
            index = end;
        }
    }
    Ok(nodes)
}

/// The byte offset of the `>` that closes a tag, ignoring `>` inside quotes.
fn find_tag_end(xml: &str, from: usize) -> Result<usize, String> {
    let mut quote = None;
    for (offset, ch) in xml[from..].char_indices() {
        match quote {
            Some(current) => {
                if ch == current {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '>' => return Ok(from + offset),
                '<' => return Err(format!("malformed tag near byte {}", from + offset)),
                _ => {}
            },
        }
    }
    Err(format!("unterminated tag near byte {from}"))
}

fn parse_open(inner: &str, at: usize) -> Result<Tag, String> {
    let mut body = inner.trim();
    if body.is_empty() {
        return Err(format!("empty tag at byte {at}"));
    }
    let self_closing = body.ends_with('/');
    if self_closing {
        body = body[..body.len() - 1].trim_end();
    }
    let end = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = &body[..end];
    if name.is_empty() || name.contains('/') {
        return Err(format!("malformed tag at byte {at}"));
    }
    let attributes = parse_attributes(&body[end..], at)?;
    Ok(Tag {
        name: name.to_string(),
        attributes,
        closing: false,
        self_closing,
    })
}

fn parse_close(inner: &str, at: usize) -> Result<Tag, String> {
    let name = inner.trim();
    if name.is_empty() || name.contains(char::is_whitespace) || name.contains('/') {
        return Err(format!("malformed closing tag at byte {at}"));
    }
    Ok(Tag {
        name: name.to_string(),
        attributes: Vec::new(),
        closing: true,
        self_closing: false,
    })
}

fn parse_attributes(mut text: &str, at: usize) -> Result<Vec<(String, String)>, String> {
    let mut attributes: Vec<(String, String)> = Vec::new();
    loop {
        text = text.trim_start();
        if text.is_empty() {
            return Ok(attributes);
        }
        let equals = text
            .find('=')
            .ok_or_else(|| format!("attribute has no '=' in tag at byte {at}"))?;
        let name = text[..equals].trim();
        if name.is_empty()
            || name.contains(char::is_whitespace)
            || name.contains('"')
            || name.contains('\'')
        {
            return Err(format!("malformed attribute name in tag at byte {at}"));
        }
        let value = text[equals + 1..].trim_start();
        let quote = value
            .chars()
            .next()
            .filter(|ch| *ch == '"' || *ch == '\'')
            .ok_or_else(|| format!("unquoted attribute {name} in tag at byte {at}"))?;
        let value = &value[quote.len_utf8()..];
        let close = value
            .find(quote)
            .ok_or_else(|| format!("unterminated attribute {name} in tag at byte {at}"))?;
        attributes.push((name.to_string(), decode_entities(&value[..close])?));
        text = &value[close + quote.len_utf8()..];
    }
}

fn decode_entities(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after
            .find(';')
            .ok_or_else(|| format!("unterminated entity in {text:?}"))?;
        let name = &after[..semi];
        let decoded = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => return Err(format!("unsupported entity &{name};")),
        };
        out.push(decoded);
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Check that tags are balanced and that the document has one root element.
pub(super) fn validate(nodes: &[Node]) -> Result<(), String> {
    let mut stack: Vec<&str> = Vec::new();
    let mut roots = 0;
    for node in nodes {
        match node {
            Node::Text(text) => {
                if stack.is_empty() && !text.trim().is_empty() {
                    return Err("text outside the root element".to_string());
                }
            }
            Node::Tag(tag) if tag.closing => {
                let open = stack
                    .pop()
                    .ok_or_else(|| format!("unexpected </{}>", tag.name))?;
                if open != tag.name {
                    return Err(format!("</{}> closes <{open}>", tag.name));
                }
            }
            Node::Tag(tag) => {
                if stack.is_empty() {
                    roots += 1;
                }
                if !tag.self_closing {
                    stack.push(&tag.name);
                }
            }
        }
    }
    if let Some(open) = stack.last() {
        return Err(format!("truncated document: <{open}> is not closed"));
    }
    if roots == 0 {
        return Err("document has no elements".to_string());
    }
    if roots > 1 {
        return Err("document has more than one root element".to_string());
    }
    Ok(())
}

/// Pair every opening tag with the index of its closing tag.
pub(super) fn matching(nodes: &[Node]) -> Vec<Option<usize>> {
    let mut closes = vec![None; nodes.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let Node::Tag(tag) = node else {
            continue;
        };
        if tag.closing {
            if let Some(open) = stack.pop() {
                closes[open] = Some(index);
            }
        } else if !tag.self_closing {
            stack.push(index);
        }
    }
    closes
}
