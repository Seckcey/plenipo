//! Plain text from what services send: a message written as a web page (Teams), and a Word
//! document's words. Only reading: nothing is run, no link is followed, no picture is fetched.

use std::io::Read as _;

/// The most a Word document may be to be read (bytes), and the most text it gives.
pub const MAX_DOCX_BYTES: usize = 4 * 1024 * 1024;
const MAX_DOCX_XML: u64 = 16 * 1024 * 1024;

/// A web page's text: tags dropped, line breaks kept, the common character names decoded.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(end) = rest[i..].find('>') else {
            // A "<" that opens no tag is text.
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let tag = rest[i + 1..i + end].trim().to_ascii_lowercase();
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        if matches!(
            name,
            "br" | "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "blockquote"
        ) && !out.ends_with('\n')
        {
            out.push('\n');
        }
        rest = &rest[i + end + 1..];
    }
    out.push_str(rest);
    decode_entities(&out)
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&#39;`, `&nbsp;`, and numeric ones.
pub fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').filter(|e| *e <= 10);
        let decoded = end.and_then(|e| {
            let name = &after[..e];
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            }
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A Word document's words (`word/document.xml`): one line per paragraph. `Err`: why not.
pub fn docx_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_DOCX_BYTES {
        return Err("the document is too big to read here".into());
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "it is not a Word document Plenipo can read".to_owned())?;
    let entry = zip
        .by_name("word/document.xml")
        .map_err(|_| "it is not a Word document Plenipo can read".to_owned())?;
    let mut xml = String::new();
    entry
        .take(MAX_DOCX_XML)
        .read_to_string(&mut xml)
        .map_err(|_| "its words could not be read".to_owned())?;
    let mut out = String::new();
    let mut rest = xml.as_str();
    while let Some(i) = rest.find('<') {
        let Some(end) = rest[i..].find('>') else {
            break;
        };
        let tag = &rest[i + 1..i + end];
        let after = &rest[i + end + 1..];
        if (tag == "w:t" || tag.starts_with("w:t ")) && !tag.ends_with('/') {
            let close = after.find("</w:t>").unwrap_or(after.len());
            out.push_str(&decode_entities(&after[..close]));
            rest = &after[close..];
            continue;
        }
        if tag == "/w:p" {
            out.push('\n');
        } else if tag == "w:tab/" {
            out.push('\t');
        } else if tag == "w:br/" {
            out.push('\n');
        }
        rest = after;
    }
    Ok(out.trim().to_owned())
}

/// The most notes [`markup_notes`] gives.
const MAX_MARKUP_NOTES: usize = 30;

/// What a web page's markup holds that its text does not show: where each link goes, where
/// each picture, frame, or script comes from, forms, and code run on a click or a load. For an
/// approval card, so the owner sees what publishing puts on the site (ADR-071 §6.7). One line
/// each, at most 30.
pub fn markup_notes(html: &str) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();
    let mut add = |note: String| {
        let note: String = note
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(240)
            .collect();
        if !notes.contains(&note) && notes.len() < MAX_MARKUP_NOTES {
            notes.push(note);
        }
    };
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        let Some(end) = rest[i..].find('>') else {
            break;
        };
        let tag = &rest[i + 1..i + end];
        rest = &rest[i + end + 1..];
        if tag.starts_with('/') || tag.starts_with('!') {
            continue;
        }
        let name_end = tag
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        match name.as_str() {
            "script" => add("a script: publishing runs it for every visitor".into()),
            "iframe" | "frame" | "object" | "embed" => add(format!("an embedded <{name}>")),
            "form" => add("a form".into()),
            "style" | "link" | "meta" | "base" => add(format!("a <{name}> tag")),
            _ => {}
        }
        for (attr, value) in attributes(&tag[name_end..]) {
            let what = match attr.as_str() {
                "href" | "xlink:href" => "a link to",
                "src" | "srcset" | "data" | "poster" | "background" => "content from",
                "action" | "formaction" => "a form that sends to",
                a if a.starts_with("on") && a.len() > 2 => {
                    add(format!("code run on \"{}\" ({a})", &a[2..]));
                    continue;
                }
                "style"
                    if {
                        let v = value.to_ascii_lowercase().replace(' ', "");
                        v.contains("display:none") || v.contains("visibility:hidden")
                    } =>
                {
                    add("text hidden from visitors (a style)".into());
                    continue;
                }
                _ => continue,
            };
            let value = decode_entities(value.trim());
            if !value.is_empty() {
                add(format!("{what} {value}"));
            }
        }
    }
    notes
}

/// A tag's attributes, as (name in small letters, value).
fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        if start == i {
            break;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let v0 = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = s[v0..i].to_owned();
                i = (i + 1).min(b.len());
            } else {
                let v0 = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = s[v0..i].to_owned();
            }
        }
        out.push((name, value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_written_as_a_page_becomes_its_words() {
        assert_eq!(
            html_to_text("<div><p>Hi &amp; welcome</p><p>Line&nbsp;two<br>three</p></div>"),
            "Hi & welcome\nLine two\nthree"
        );
        assert_eq!(html_to_text("a < b"), "a < b");
        assert_eq!(decode_entities("&#65;&#x42;&bogus;"), "AB&bogus;");
    }

    #[test]
    fn a_word_documents_words_are_read() {
        use std::io::Write as _;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                b"<w:document><w:body><w:p><w:r><w:t>Quarterly</w:t></w:r><w:r>\
                  <w:t xml:space=\"preserve\"> plan &amp; budget</w:t></w:r></w:p>\
                  <w:p><w:r><w:t>Second</w:t></w:r></w:p></w:body></w:document>",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(
            docx_text(buf.get_ref()).unwrap(),
            "Quarterly plan & budget\nSecond"
        );
        assert!(docx_text(b"not a zip").is_err());
    }

    #[test]
    fn markup_the_text_does_not_show_is_named() {
        let html = "<p>Holiday hours</p><a href=\"https://phish.example/pay\">Pay your invoice</a>\
            <script src='https://evil.example/x.js'></script><img src=x onerror=alert(1)>\
            <div style=\"display: none\">hidden</div><iframe src=\"https://frame.example\"></iframe>";
        let notes = markup_notes(html);
        for want in [
            "a link to https://phish.example/pay",
            "a script: publishing runs it for every visitor",
            "content from https://evil.example/x.js",
            "content from x",
            "code run on \"error\" (onerror)",
            "text hidden from visitors (a style)",
            "an embedded <iframe>",
            "content from https://frame.example",
        ] {
            assert!(notes.iter().any(|n| n == want), "{want}: {notes:?}");
        }
        assert!(markup_notes("<p>Just <b>words</b>.</p>").is_empty());
    }
}
