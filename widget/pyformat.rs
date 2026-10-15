//! Python's `str.format` for the widget's message templates.
//!
//! The original formats translated templates such as
//! `"Monitor script failed with code {0}.\n\nDetails: {1}"`, so a translation
//! decides the placeholders. Positional fields (`{}`, `{0}`), `{{`/`}}` and the
//! `!s` conversion behave like Python; an error carries the text of the
//! exception Python raises (`str(e)`), which the original sometimes shows in a
//! dialog. Fields Python would resolve differently (keyword or attribute
//! fields, `!r`/`!a`, format specs) are rejected; no dictionary uses them.

/// `template.format(*args)`.
pub fn format(template: &str, args: &[&str]) -> Result<String, String> {
    let chars: Vec<char> = template.chars().collect();
    let mut output = String::with_capacity(template.len());
    let mut position = 0;
    let mut numbering = Numbering::Unknown;
    while position < chars.len() {
        let c = chars[position];
        position += 1;
        if c != '{' && c != '}' {
            output.push(c);
            continue;
        }
        let at_end = position >= chars.len();
        if c == '}' && (at_end || chars[position] != '}') {
            return Err("Single '}' encountered in format string".into());
        }
        if c == '{' && at_end {
            return Err("Single '{' encountered in format string".into());
        }
        if chars[position] == c {
            // `{{` or `}}`
            output.push(c);
            position += 1;
            continue;
        }
        let field = parse_field(&chars, &mut position)?;
        let index = match field.name.as_str() {
            "" => numbering.automatic()?,
            name if name.bytes().all(|b| b.is_ascii_digit()) => {
                numbering.manual()?;
                // Python parses the digits as a Py_ssize_t.
                name.parse::<usize>()
                    .map_err(|_| "Too many decimal digits in format string".to_owned())?
            }
            name => return Err(format!("unsupported replacement field {{{name}}}")),
        };
        if !matches!(field.conversion, None | Some('s')) || !field.spec.is_empty() {
            return Err(format!("unsupported replacement field in {template:?}"));
        }
        let value = args.get(index).ok_or_else(|| {
            format!("Replacement index {index} out of range for positional args tuple")
        })?;
        output.push_str(value);
    }
    Ok(output)
}

enum Numbering {
    Unknown,
    Automatic(usize),
    Manual,
}

impl Numbering {
    fn automatic(&mut self) -> Result<usize, String> {
        match self {
            Self::Manual => Err(
                "cannot switch from manual field specification to automatic field numbering".into(),
            ),
            Self::Unknown => {
                *self = Self::Automatic(1);
                Ok(0)
            }
            Self::Automatic(next) => {
                *next += 1;
                Ok(*next - 1)
            }
        }
    }

    fn manual(&mut self) -> Result<(), String> {
        match self {
            Self::Automatic(_) => Err(
                "cannot switch from automatic field numbering to manual field specification".into(),
            ),
            _ => {
                *self = Self::Manual;
                Ok(())
            }
        }
    }
}

struct Field {
    name: String,
    conversion: Option<char>,
    spec: String,
}

/// CPython's `parse_field`, starting after the opening `{`.
fn parse_field(chars: &[char], position: &mut usize) -> Result<Field, String> {
    let mut field = Field {
        name: String::new(),
        conversion: None,
        spec: String::new(),
    };
    let mut terminator = None;
    while *position < chars.len() {
        let c = chars[*position];
        *position += 1;
        match c {
            '{' => return Err("unexpected '{' in field name".into()),
            '[' => {
                field.name.push(c);
                while *position < chars.len() && chars[*position] != ']' {
                    field.name.push(chars[*position]);
                    *position += 1;
                }
            }
            '}' | ':' | '!' => {
                terminator = Some(c);
                break;
            }
            _ => field.name.push(c),
        }
    }
    match terminator {
        Some('}') => Ok(field),
        Some(c @ ('!' | ':')) => {
            if c == '!' {
                let Some(&conversion) = chars.get(*position) else {
                    return Err("end of string while looking for conversion specifier".into());
                };
                field.conversion = Some(conversion);
                *position += 1;
                if let Some(&next) = chars.get(*position) {
                    *position += 1;
                    if next == '}' {
                        return Ok(field);
                    }
                    if next != ':' {
                        return Err("expected ':' after conversion specifier".into());
                    }
                }
            }
            let mut depth = 1;
            while *position < chars.len() {
                let c = chars[*position];
                *position += 1;
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(field);
                        }
                    }
                    _ => {}
                }
                field.spec.push(c);
            }
            Err("unmatched '{' in format spec".into())
        }
        _ => Err("expected '}' before end of string".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn formats_like_python() {
        assert_eq!(format("a {} b {}", &["1", "2"]).unwrap(), "a 1 b 2");
        assert_eq!(format("{1}-{0}-{1}", &["x", "y"]).unwrap(), "y-x-y");
        assert_eq!(format("{{}} {}", &["v"]).unwrap(), "{} v");
        assert_eq!(format("}}{{", &[]).unwrap(), "}{");
        assert_eq!(format("{!s}", &["v"]).unwrap(), "v");
        assert_eq!(format("{0!s:}", &["v"]).unwrap(), "v");
        assert_eq!(
            format(
                "Monitor script failed with code {0}.\n\nDetails: {1}",
                &["3", "boom"]
            )
            .unwrap(),
            "Monitor script failed with code 3.\n\nDetails: boom"
        );
        assert_eq!(format("Zażółć {}", &["gęślą"]).unwrap(), "Zażółć gęślą");
    }

    #[test]
    fn errors_carry_python_messages() {
        let error = |template: &str, args: &[&str]| format(template, args).unwrap_err();
        assert_eq!(error("a }", &[]), "Single '}' encountered in format string");
        assert_eq!(error("a {", &[]), "Single '{' encountered in format string");
        assert_eq!(error("a {0", &["x"]), "expected '}' before end of string");
        assert_eq!(error("{a{}", &[]), "unexpected '{' in field name");
        assert_eq!(
            error("{} {}", &["x"]),
            "Replacement index 1 out of range for positional args tuple"
        );
        assert_eq!(
            error("{0} {}", &["x", "y"]),
            "cannot switch from manual field specification to automatic field numbering"
        );
        assert_eq!(
            error("{} {0}", &["x", "y"]),
            "cannot switch from automatic field numbering to manual field specification"
        );
        assert_eq!(
            error("{!", &[]),
            "end of string while looking for conversion specifier"
        );
        assert_eq!(
            error("{!sx}", &[]),
            "expected ':' after conversion specifier"
        );
        assert_eq!(error("{:", &[]), "unmatched '{' in format spec");
    }
}
