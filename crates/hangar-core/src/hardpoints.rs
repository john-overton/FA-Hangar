//! Source-coordinate PT stations. Geometry and executable sections are untouched.
use crate::{
    brf::{Brf, Field},
    invalid, Result,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[derive(Clone, Debug)]
pub struct Station {
    pub position: [i32; 3],
    pub store: Option<String>,
    pub fields: Vec<usize>,
}
fn number(f: &Field) -> Result<i32> {
    if let Some(v) = f.value.strip_prefix('$') {
        let n = u32::from_str_radix(v, 16).map_err(|_| invalid("Invalid station integer"))?;
        Ok(if f.kind == "word" {
            n as u16 as i16 as i32
        } else {
            n as i32
        })
    } else {
        f.value
            .parse()
            .map_err(|_| invalid("Invalid station integer"))
    }
}
fn header(brf: &Brf) -> Result<(usize, usize)> {
    let count = brf
        .fields
        .iter()
        .position(|f| f.label == "npc.numHards")
        .ok_or("Unrecognized PT station schema")?;
    let ptr = brf
        .fields
        .iter()
        .position(|f| f.label == "npc.hards")
        .ok_or("Unrecognized PT station pointer")?;
    if brf
        .issues
        .iter()
        .any(|s| s.to_ascii_lowercase().contains("hardpoint"))
    {
        return Err(invalid(
            "Correct the hardpoint count/record diagnostics first",
        ));
    }
    Ok((count, ptr))
}
pub fn read(brf: &Brf) -> Result<Vec<Station>> {
    let (count, ptr) = header(brf)?;
    let n = number(&brf.fields[count])?;
    if !(0..=64).contains(&n) {
        return Err(invalid("Station count outside 0..64"));
    }
    let block = &brf.fields[ptr].value;
    let rows: Vec<_> = brf
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.block == *block)
        .map(|(i, _)| i)
        .collect();
    if rows.len() != n as usize * 12 {
        return Err(invalid("Station table size does not match its count"));
    }
    let mut out = Vec::new();
    for row in rows.chunks_exact(12) {
        let f = &brf.fields[row[8]];
        let store = if f.kind == "ptr" {
            let strings: Vec<_> = brf
                .fields
                .iter()
                .filter(|s| s.block == f.value && s.kind == "string")
                .collect();
            if strings.len() != 1 {
                return Err(invalid("Unrecognized station store reference"));
            }
            Some(strings[0].value.trim_matches('"').to_ascii_uppercase())
        } else {
            None
        };
        out.push(Station {
            position: [
                number(&brf.fields[row[1]])?,
                number(&brf.fields[row[2]])?,
                number(&brf.fields[row[3]])?,
            ],
            store,
            fields: row.to_vec(),
        });
    }
    Ok(out)
}
fn patch(bytes: &[u8], mut edits: Vec<(usize, usize, String)>) -> Result<Vec<u8>> {
    edits.sort_unstable_by_key(|(a, _, _)| core::cmp::Reverse(*a));
    let mut out = bytes.to_vec();
    let mut last = bytes.len() + 1;
    for (a, b, value) in edits {
        if a > b || b > bytes.len() || b > last {
            return Err(invalid("Overlapping station edits"));
        }
        last = a;
        out.splice(a..b, value.bytes());
    }
    let brf = Brf::parse(&out, "PT")?;
    read(&brf)?;
    Ok(out)
}
fn newline(bytes: &[u8]) -> &'static str {
    if bytes.windows(2).any(|w| w == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}
fn fresh(brf: &Brf, prefix: &str, bytes: &[u8]) -> Result<String> {
    let text = core::str::from_utf8(bytes).map_err(|_| invalid("Invalid BRF text"))?;
    for i in 0..4096 {
        let label = format!("{prefix}{i}");
        if !brf.fields.iter().any(|f| f.block == label)
            && !text
                .lines()
                .any(|l| l.split(';').next().unwrap_or("").trim() == format!(":{label}"))
        {
            return Ok(label);
        }
    }
    Err(invalid("No free station block label"))
}
fn row_bounds(bytes: &[u8], brf: &Brf, row: &Station) -> (usize, usize) {
    let start = brf.fields[row.fields[0]].kind_start;
    let end = brf.fields[*row.fields.last().unwrap()].end;
    let a = bytes[..start]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |p| p + 1);
    let b = bytes[end..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(end, |p| end + p + 1);
    (a, b)
}
pub fn position(bytes: &[u8], station: usize, xyz: [i32; 3]) -> Result<Vec<u8>> {
    if xyz.iter().any(|v| !(-32768..=32767).contains(v)) {
        return Err(invalid(
            "Station coordinates must fit signed 16-bit source values",
        ));
    }
    let b = Brf::parse(bytes, "PT")?;
    let rows = read(&b)?;
    let row = rows.get(station).ok_or("No selected station")?;
    b.edit_many(
        bytes,
        &(0..3)
            .filter(|i| row.position[*i] != xyz[*i])
            .map(|i| (row.fields[i + 1], xyz[i].to_string()))
            .collect::<Vec<_>>(),
        "PT",
    )
}
pub fn add(bytes: &[u8], donor: Option<usize>, xyz: [i32; 3]) -> Result<Vec<u8>> {
    let b = Brf::parse(bytes, "PT")?;
    let rows = read(&b)?;
    if rows.len() >= 64 {
        return Err(invalid("At most 64 stations"));
    }
    let (count, ptr) = header(&b)?;
    let p = &b.fields[ptr];
    if p.kind == "ptr"
        && b.fields
            .iter()
            .filter(|f| f.kind == "ptr" && f.value == p.value)
            .count()
            > 1
    {
        return Err(invalid(
            "Station block is shared by other pointers; isolate it before resizing",
        ));
    }
    let nl = newline(bytes);
    let mut record = String::new();
    if let Some(i) = donor {
        let row = rows.get(i).ok_or("No donor station")?;
        let (a, z) = row_bounds(bytes, &b, row);
        record.push_str(core::str::from_utf8(&bytes[a..z]).unwrap());
    } else {
        for (kind, name) in crate::schema::HARDPOINT {
            let k = if *kind == "ptr" { "dword" } else { kind };
            record.push_str(&format!(
                "{k} {} ; {name}{nl}",
                if *name == "maxItems" { 1 } else { 0 }
            ));
        }
    }
    let f = &b.fields[count];
    let mut edits = vec![(f.start, f.end, format!("{}", rows.len() + 1))];
    if p.kind == "ptr" {
        let end = if let Some(last) = rows.last() {
            row_bounds(bytes, &b, last).1
        } else {
            return Err(invalid(
                "Empty non-null station block is not supported for insertion",
            ));
        };
        edits.push((end, end, record));
    } else {
        let label = fresh(&b, "HG_HARDS", bytes)?;
        edits.push((p.kind_start, p.end, format!("ptr {label}")));
        edits.push((b.end_offset, b.end_offset, format!(":{label}{nl}{record}")));
    }
    let out = patch(bytes, edits)?;
    position(&out, rows.len(), xyz)
}
pub fn remove(bytes: &[u8], station: usize) -> Result<Vec<u8>> {
    let b = Brf::parse(bytes, "PT")?;
    let rows = read(&b)?;
    if rows.len() <= 1 {
        return Err(invalid(
            "Keep at least one station; clear its store instead",
        ));
    }
    let (count, ptr) = header(&b)?;
    let p = &b.fields[ptr];
    if b.fields
        .iter()
        .filter(|f| f.kind == "ptr" && f.value == p.value)
        .count()
        != 1
    {
        return Err(invalid("Station block has shared pointers"));
    }
    let row = rows.get(station).ok_or("No selected station")?;
    let (a, z) = row_bounds(bytes, &b, row);
    let f = &b.fields[count];
    patch(
        bytes,
        vec![
            (a, z, String::new()),
            (f.start, f.end, format!("{}", rows.len() - 1)),
        ],
    )
}
pub fn store(bytes: &[u8], station: usize, name: &str) -> Result<Vec<u8>> {
    let b = Brf::parse(bytes, "PT")?;
    let rows = read(&b)?;
    let row = rows.get(station).ok_or("No selected station")?;
    let f = &b.fields[row.fields[8]];
    let name = name.trim().to_ascii_uppercase();
    if row.store.as_deref() == Some(name.as_str()) || row.store.is_none() && name.is_empty() {
        return Ok(bytes.to_vec());
    }
    if name.is_empty() {
        return patch(bytes, vec![(f.kind_start, f.end, "dword 0".into())]);
    }
    crate::archive::validate_name(&name)?;
    if !matches!(name.rsplit('.').next(), Some("JT" | "SEE" | "ECM" | "GAS")) {
        return Err(invalid("Store must be a JT, SEE, ECM or GAS resource"));
    }
    let label = fresh(&b, "HG_STORE", bytes)?;
    let nl = newline(bytes);
    patch(
        bytes,
        vec![
            (f.kind_start, f.end, format!("ptr {label}")),
            (
                b.end_offset,
                b.end_offset,
                format!(":{label}{nl}string \"{name}\"{nl}"),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn station_edits_preserve_other_records_and_resolve_new_store_blocks() {
        let original = crate::brf::demo_with_records();
        let parsed = Brf::parse(&original, "PT").unwrap();
        assert_eq!(read(&parsed).unwrap().len(), 2);
        assert_eq!(position(&original, 0, [-50, 0, 0]).unwrap(), original);
        let moved = position(&original, 0, [12, -8, 9]).unwrap();
        let after = Brf::parse(&moved, "PT").unwrap();
        for (a, b) in parsed.fields.iter().zip(&after.fields) {
            if ![
                "hardpoint[0].pos.x",
                "hardpoint[0].pos.y",
                "hardpoint[0].pos.z",
            ]
            .contains(&a.label.as_str())
            {
                assert_eq!(a.value, b.value);
            }
        }
        let stored = store(&moved, 0, "AIM9.JT").unwrap();
        assert_eq!(
            read(&Brf::parse(&stored, "PT").unwrap()).unwrap()[0]
                .store
                .as_deref(),
            Some("AIM9.JT")
        );
        let added = add(&stored, Some(0), [20, 30, 40]).unwrap();
        let rows = read(&Brf::parse(&added, "PT").unwrap()).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].store.as_deref(), Some("AIM9.JT"));
        let changed = store(&added, 2, "RADAR.SEE").unwrap();
        let rows = read(&Brf::parse(&changed, "PT").unwrap()).unwrap();
        assert_eq!(rows[0].store.as_deref(), Some("AIM9.JT"));
        assert_eq!(rows[2].store.as_deref(), Some("RADAR.SEE"));
        let cleared = store(&changed, 2, "").unwrap();
        assert!(read(&Brf::parse(&cleared, "PT").unwrap()).unwrap()[2]
            .store
            .is_none());
        let removed = remove(&added, 2).unwrap();
        assert_eq!(removed, stored);
        assert!(position(&original, 0, [32768, 0, 0]).is_err());
        assert!(store(&original, 0, "BAD.SH").is_err());
    }
    #[test]
    fn first_station_and_shared_block_guards() {
        let empty = crate::brf::demo();
        let one = add(&empty, None, [1, 2, 3]).unwrap();
        assert_eq!(
            read(&Brf::parse(&one, "PT").unwrap()).unwrap()[0].position,
            [1, 2, 3]
        );
        assert!(remove(&one, 0).is_err());
        let source = String::from_utf8(crate::brf::demo_with_records())
            .unwrap()
            .replace("end\r\n", ":alias\r\nptr stations\r\nend\r\n");
        assert!(add(source.as_bytes(), None, [0; 3]).is_err());
        assert!(remove(source.as_bytes(), 0).is_err());
    }
}
