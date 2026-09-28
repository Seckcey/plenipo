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
}
