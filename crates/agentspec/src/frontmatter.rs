use crate::error::{AppError, Result};

pub struct Parsed {
    pub frontmatter: String,
    pub body: String,
}

/// True for a fence line: exactly `---` ignoring trailing whitespace/CR.
/// `----` (thematic break) and `--- text` are not fences.
fn is_fence(line: &str) -> bool {
    line.trim_end() == "---"
}

pub fn parse(content: &str) -> Result<Parsed> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let trimmed = content.trim_start();

    let (first_line, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    if !is_fence(first_line) {
        return Err(AppError::InvalidFrontmatter(
            "file must start with ---".into(),
        ));
    }

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if is_fence(line.trim_end_matches('\n')) {
            let frontmatter = rest[..offset].trim().to_string();
            let body = rest[offset + line.len()..].trim_start().to_string();
            return Ok(Parsed { frontmatter, body });
        }
        offset += line.len();
    }

    Err(AppError::InvalidFrontmatter("no closing --- found".into()))
}

/// Compose a frontmatter document from already-serialized YAML and a body.
/// Inverse of [`parse`] up to surrounding whitespace.
pub fn compose(frontmatter_yaml: &str, body: &str) -> String {
    let yaml = frontmatter_yaml.trim_end();
    if body.is_empty() {
        format!("---\n{yaml}\n---\n")
    } else {
        format!("---\n{yaml}\n---\n\n{body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_handles_fence_edge_cases() {
        struct Case {
            name: &'static str,
            input: &'static str,
            expect: Option<(&'static str, &'static str)>,
        }
        let cases = [
            Case {
                name: "simple document",
                input: "---\nname: x\n---\nbody\n",
                expect: Some(("name: x", "body\n")),
            },
            Case {
                name: "crlf line endings",
                input: "---\r\nname: x\r\n---\r\nbody\r\n",
                expect: Some(("name: x", "body\r\n")),
            },
            Case {
                name: "blank lines before opener",
                input: "\n\n---\nname: x\n---\nbody\n",
                expect: Some(("name: x", "body\n")),
            },
            Case {
                name: "bom before opener",
                input: "\u{feff}---\nname: x\n---\nbody\n",
                expect: Some(("name: x", "body\n")),
            },
            Case {
                name: "empty frontmatter",
                input: "---\n---\nbody\n",
                expect: Some(("", "body\n")),
            },
            Case {
                name: "closer at eof without newline",
                input: "---\nname: x\n---",
                expect: Some(("name: x", "")),
            },
            Case {
                name: "closer with trailing spaces",
                input: "---\nname: x\n---  \nbody\n",
                expect: Some(("name: x", "body\n")),
            },
            Case {
                name: "body keeps later fence lines",
                input: "---\nname: x\n---\nintro\n---\noutro\n",
                expect: Some(("name: x", "intro\n---\noutro\n")),
            },
            Case {
                name: "thematic break is not a closer",
                input: "---\nname: x\n----\nbody\n",
                expect: None,
            },
            Case {
                name: "indented dashes are not a closer",
                input: "---\nname: x\n  ---\nbody\n",
                expect: None,
            },
            Case {
                name: "opener with trailing text",
                input: "--- yaml\nname: x\n---\nbody\n",
                expect: None,
            },
            Case {
                name: "no opener",
                input: "name: x\n---\nbody\n",
                expect: None,
            },
            Case {
                name: "no closer",
                input: "---\nname: x\n",
                expect: None,
            },
        ];

        for case in cases {
            match (parse(case.input), case.expect) {
                (Ok(parsed), Some((fm, body))) => {
                    assert_eq!(parsed.frontmatter, fm, "{}: frontmatter", case.name);
                    assert_eq!(parsed.body, body, "{}: body", case.name);
                }
                (Ok(parsed), None) => panic!(
                    "{}: expected error, got frontmatter {:?} body {:?}",
                    case.name, parsed.frontmatter, parsed.body
                ),
                (Err(e), Some(_)) => panic!("{}: expected success, got {e}", case.name),
                (Err(_), None) => {}
            }
        }
    }

    #[test]
    fn compose_then_parse_roundtrips() {
        let doc = compose("name: x\ndescription: y", "# Heading\n\nBody text.\n");
        let parsed = parse(&doc).unwrap();
        assert_eq!(parsed.frontmatter, "name: x\ndescription: y");
        assert_eq!(parsed.body, "# Heading\n\nBody text.\n");

        let empty_body = compose("name: x\n", "");
        let parsed = parse(&empty_body).unwrap();
        assert_eq!(parsed.frontmatter, "name: x");
        assert_eq!(parsed.body, "");
    }
}
