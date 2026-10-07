//! Lossless BRF operand editing: comments, whitespace, labels and unknown fields survive.
use crate::{invalid, schema, Result};
use alloc::{collections::BTreeSet, string::String, vec::Vec};
#[derive(Clone, Debug)]
pub struct Field {
    pub label: String,
    pub block: String,
    pub kind: String,
    pub value: String,
    pub start: usize,
    pub kind_start: usize,
    pub end: usize,
    pub scaled: bool,
}
#[derive(Clone, Debug)]
pub struct Brf {
    pub fields: Vec<Field>,
    pub issues: Vec<String>,
    pub end_offset: usize,
}
fn number(kind: &str, value: &str) -> Result<()> {
    let n = if let Some(s) = value.strip_prefix('$') {
        i64::from_str_radix(s, 16)
    } else {
        value.parse::<i64>()
    }
    .map_err(|_| invalid("Invalid integer; use decimal or $hex"))?;
    // Retail uses sign-extended hex words such as $ffff8000. Preserve those on read.
    let valid = match kind {
        "byte" => (-128..=255).contains(&n),
        "word" => {
            (-32768..=65535).contains(&n)
                || (value.starts_with('$') && (0xffff8000..=0xffffffff).contains(&n))
        }
        "dword" => (i32::MIN as i64..=u32::MAX as i64).contains(&n),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("Value outside field width"))
    }
}
impl Brf {
    pub fn parse(bytes: &[u8], extension: &str) -> Result<Self> {
        if bytes.len() > 1024 * 1024 {
            return Err(invalid("BRF exceeds 1 MiB limit"));
        }
        let text =
            core::str::from_utf8(bytes).map_err(|_| invalid("BRF text is not UTF-8/ASCII"))?;
        if !text.starts_with("[brent's_relocatable_format]") {
            return Err(invalid("Not a textual BRF definition"));
        }
        let mut fields = Vec::new();
        let mut block = String::new();
        let mut labels = BTreeSet::new();
        let mut ended = false;
        let mut end_offset = 0;
        let mut pos = 0;
        for (line_no, line) in text.split_inclusive('\n').enumerate() {
            let at = pos;
            pos += line.len();
            if line_no == 0 {
                continue;
            }
            // Semicolons inside a quoted string are data, not comments.
            let mut quoted = false;
            let mut cut = line.len();
            for (i, c) in line.char_indices() {
                if c == '"' {
                    quoted = !quoted;
                }
                if c == ';' && !quoted {
                    cut = i;
                    break;
                }
            }
            let content = line[..cut].trim();
            if content.is_empty() {
                continue;
            }
            if ended {
                return Err(invalid("BRF data after end"));
            }
            if content == "end" {
                ended = true;
                end_offset = at;
                continue;
            }
            if let Some(label) = content.strip_prefix(':') {
                if label.is_empty()
                    || !label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    || !labels.insert(label.to_string())
                {
                    return Err(invalid("Invalid or duplicate BRF label"));
                }
                block = label.into();
                continue;
            }
            let (kind, value) = content
                .split_once(char::is_whitespace)
                .ok_or_else(|| invalid("Invalid BRF statement"))?;
            let value = value.trim();
            let start = at + (value.as_ptr() as usize - line.as_ptr() as usize);
            let scaled = value.starts_with('^');
            let v = value.strip_prefix('^').unwrap_or(value);
            match kind {
                "byte" | "word" | "dword" => number(kind, v)?,
                "string" => {
                    if !v.starts_with('"')
                        || !v.ends_with('"')
                        || v.len() < 2
                        || v[1..v.len() - 1].contains('"')
                    {
                        return Err(invalid("Invalid BRF string"));
                    }
                }
                "ptr" | "symbol" => {
                    if v.is_empty() || v.contains(char::is_whitespace) {
                        return Err(invalid("Invalid BRF reference"));
                    }
                }
                _ => return Err(invalid("Unknown BRF statement")),
            }
            fields.push(Field {
                label: format!(
                    "{}[{}]",
                    if block.is_empty() { "root" } else { &block },
                    fields.len()
                ),
                block: block.clone(),
                kind: kind.into(),
                value: v.into(),
                start: start + usize::from(scaled),
                kind_start: at + (kind.as_ptr() as usize - line.as_ptr() as usize),
                end: start + value.len(),
                scaled,
            });
        }
        if !ended {
            return Err(invalid("Unterminated BRF"));
        }
        for f in &fields {
            if f.kind == "ptr" && !labels.contains(&f.value) {
                return Err(invalid("Unresolved BRF pointer"));
            }
        }
        let root_len = fields.iter().take_while(|f| f.block.is_empty()).count();
        let mut layout = Vec::new();
        match extension.to_ascii_uppercase().as_str() {
            "PT" | "NT" => {
                for (prefix, s) in [
                    ("object", schema::OBJECT),
                    ("npc", schema::NPC),
                    ("plane", schema::PLANE),
                ] {
                    if prefix == "plane" && extension.eq_ignore_ascii_case("NT") {
                        continue;
                    }
                    for (kind, name) in s {
                        layout.push((prefix, *kind, *name));
                    }
                }
            }
            "JT" => {
                for (prefix, s) in [
                    ("object", schema::OBJECT),
                    ("projectile", schema::PROJECTILE),
                ] {
                    for (kind, name) in s {
                        layout.push((prefix, *kind, *name));
                    }
                }
            }
            "OT" => {
                for (kind, name) in schema::OBJECT {
                    layout.push(("object", *kind, *name));
                }
            }
            "SEE" => {
                for (kind, name) in schema::SENSOR {
                    layout.push(("sensor", *kind, *name));
                }
            }
            "ECM" => {
                for (kind, name) in schema::ECM {
                    layout.push(("ecm", *kind, *name));
                }
            }
            _ => {}
        }
        if root_len == layout.len()
            && fields.iter().zip(&layout).all(|(f, (_, k, _))| {
                f.kind == *k || (*k == "ptr" && f.kind == "dword" && f.value == "0")
            })
        {
            for (f, (prefix, _, name)) in fields.iter_mut().zip(layout) {
                f.label = format!("{prefix}.{name}");
            }
        }
        let issues = crate::definition::annotate(&mut fields);
        Ok(Self {
            fields,
            issues,
            end_offset,
        })
    }
    pub fn edit_many(
        &self,
        bytes: &[u8],
        edits: &[(usize, String)],
        extension: &str,
    ) -> Result<Vec<u8>> {
        let mut ordered = edits.to_vec();
        ordered.sort_unstable_by_key(|(i, _)| core::cmp::Reverse(*i));
        let mut output = bytes.to_vec();
        let mut previous = None;
        for (i, value) in ordered {
            if previous == Some(i) {
                return Err(invalid("Duplicate field edit"));
            }
            previous = Some(i);
            let f = self.fields.get(i).ok_or("No selected field")?;
            if value.contains(['\r', '\n', ';']) {
                return Err(invalid("A field value must be one operand"));
            }
            if matches!(f.kind.as_str(), "byte" | "word" | "dword") {
                number(&f.kind, &value)?;
            } else if !matches!(f.kind.as_str(), "string" | "ptr") {
                return Err(invalid("Symbols are read-only"));
            }
            if f.end > output.len() || f.start > f.end {
                return Err(invalid("Stale field offsets"));
            }
            output.splice(f.start..f.end, value.bytes());
        }
        Self::parse(&output, extension)?;
        Ok(output)
    }
    pub fn edit(
        &self,
        bytes: &[u8],
        index: usize,
        value: &str,
        extension: &str,
    ) -> Result<Vec<u8>> {
        let f = self
            .fields
            .get(index)
            .ok_or_else(|| invalid("No selected field"))?;
        if value.contains(['\r', '\n', ';']) {
            return Err(invalid("A field value must be one operand"));
        }
        match f.kind.as_str() {
            "byte" | "word" | "dword" => number(&f.kind, value)?,
            "string" | "ptr" => {}
            _ => return Err(invalid("Symbols are read-only")),
        }
        let mut out = Vec::with_capacity(bytes.len() + value.len());
        out.extend(&bytes[..f.start]);
        out.extend(value.as_bytes());
        out.extend(&bytes[f.end..]);
        Self::parse(&out, extension)?;
        Ok(out)
    }
}
use alloc::string::ToString;
/// Synthetic, schema-shaped definition for exercising the UI without retail data.
pub fn demo() -> Vec<u8> {
    let mut s = String::from(
        "[brent's_relocatable_format]\r\n; Synthetic editor fixture, not a flyable aircraft\r\n",
    );
    for fields in [schema::OBJECT, schema::NPC, schema::PLANE] {
        for (kind, name) in fields {
            let (kind, value) = match *kind {
                "ptr" if ["ot_names", "shape", "shadowShape"].contains(name) => ("ptr", *name),
                "ptr" => ("dword", "0"),
                "symbol" => ("symbol", "_DEMO"),
                _ => (
                    *kind,
                    match *name {
                        "weight" => "10000",
                        "hitPoints" => "100",
                        "year" => "1997",
                        _ => "0",
                    },
                ),
            };
            s.push_str(&format!("    {kind} {value} ; {name}\r\n"));
        }
    }
    s.push_str(":ot_names\r\nstring \"Demo\"\r\nstring \"Synthetic aircraft\"\r\nstring \"DEMO.PT\"\r\n:shape\r\nstring \"DEMO.SH\"\r\n:shadowShape\r\nstring \"DEMO_S.SH\"\r\nend\r\n");
    s.into_bytes()
}

/// Synthetic linked station/envelope records for the structured editor demo.
pub fn demo_with_records() -> Vec<u8> {
    let mut text = String::from_utf8(demo())
        .unwrap()
        .replace("dword 0 ; hards", "ptr stations ; hards")
        .replace("byte 0 ; numHards", "byte 2 ; numHards")
        .replace("dword 0 ; env", "ptr envelope ; env")
        .replace("word 0 ; envMin", "word -1 ; envMin")
        .replace("word 0 ; envMax", "word 1 ; envMax");
    text.truncate(text.rfind("end\r\n").unwrap());
    text.push_str(":stations\r\n");
    for station in 0..2 {
        for (kind, name) in schema::HARDPOINT {
            let (kind, value) = if *kind == "ptr" {
                ("dword", String::from("0"))
            } else {
                (
                    *kind,
                    match *name {
                        "pos.x" => {
                            if station == 0 {
                                String::from("-50")
                            } else {
                                String::from("50")
                            }
                        }
                        "maxItems" => String::from("1"),
                        "name" => format!("{station}"),
                        _ => String::from("0"),
                    },
                )
            };
            text.push_str(&format!("{kind} {value} ; {name}\r\n"));
        }
    }
    text.push_str(":envelope\r\n");
    for g in -1..=1 {
        for (kind, name) in schema::ENVELOPE {
            let value = match *name {
                "gload" => format!("{g}"),
                "count" => String::from("3"),
                _ if name.starts_with("speed[") => String::from("100"),
                _ => String::from("0"),
            };
            text.push_str(&format!("{kind} {value} ; {name}\r\n"));
        }
    }
    text.push_str("end\r\n");
    text.into_bytes()
}

/// Synthetic NPC definition for editor checks; contains no game values.
pub fn demo_npc() -> Vec<u8> {
    let mut bytes = demo_with_records();
    let b = Brf::parse(&bytes, "PT").unwrap();
    for f in b
        .fields
        .iter()
        .rev()
        .filter(|f| f.label.starts_with("plane."))
    {
        bytes.drain(f.kind_start..f.end);
    }
    String::from_utf8(bytes)
        .unwrap()
        .replace("DEMO.PT", "DEMO.NT")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operand_patch_preserves_unknown_data_and_crlf() {
        let b=b"[brent's_relocatable_format]\r\n  word ^24 ; note\r\nptr name\r\n:name\r\n string \"Demo; test\"\r\nend\r\n";
        let p = Brf::parse(b, "PT").unwrap();
        let out = p.edit(b, 0, "42", "PT").unwrap();
        assert_eq!(out,b"[brent's_relocatable_format]\r\n  word ^42 ; note\r\nptr name\r\n:name\r\n string \"Demo; test\"\r\nend\r\n");
        assert!(p.edit(b, 0, "65536", "PT").is_err());
        assert!(p.edit(b, 1, "missing", "PT").is_err());
        assert!(p.edit(b, 0, "2\nend", "PT").is_err());
    }
}
