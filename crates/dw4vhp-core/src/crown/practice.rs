//! Practice-stage graduation script: the farewell enemy pair is chosen by the
//! real party size (`SYSgetPlayerNinzu() <= 2` spawns GENE-a03/a04, `>= 3`
//! spawns GENE-a05/a06, the Kokatorimon pair, and likewise for b03..b06).
//! The four-player spawn option moves both thresholds so any party takes the
//! four-player branch. Only four existing integer operands change in place.
use super::river::lua::{signed, BIAS};
use super::*;

const PLAYERS: &[u8] = b"SYSgetPlayerNinzu\0";
const MARKERS: [&[u8]; 4] = [b"GENE-a05\0", b"GENE-b06\0", b"GBOX-02\0", PLAYERS];
const PUSHINT: u32 = 6;
const CALL: u32 = 2;
const PUSHSELF: u32 = 16;
/// Retail comparisons `n <= 2` / `n >= 3`, in a05/a06 order, for both flag groups.
const RETAIL: [i64; 4] = [2, 3, 2, 3];
/// `n <= 0` never holds for a joined party and `n >= 1` always does.
const PATCHED: [i64; 4] = [0, 1, 0, 1];

/// Cheap archive-entry prefilter; [`expand`] decides whether a Lua chunk matches.
pub(super) fn candidate(b: &[u8]) -> bool {
    memchr::memmem::find(b, b"beLuaData\0").is_some() && marked(b)
}

fn marked(b: &[u8]) -> bool {
    MARKERS.iter().all(|m| memchr::memmem::find(b, m).is_some())
}

struct Function {
    strings: Vec<Range<usize>>,
    code: Range<usize>,
    end: usize,
}

fn function(b: &[u8], mut q: usize, depth: usize) -> Result<Function> {
    need(depth < 8, "practice Lua nests too deeply")?;
    q += 4 + u32at(b, q)? as usize; // source name
    q += 4; // line
    q += 5; // parameter count and vararg flag
    q += 4; // stack size
    need(
        u32at(b, q)? == 0 && u32at(b, q + 4)? == 0,
        "practice Lua has debug records",
    )?;
    q += 8;
    let count = u32at(b, q)? as usize;
    q += 4;
    need(count < 4096, "too many practice Lua strings")?;
    let mut strings = Vec::new();
    for _ in 0..count {
        let n = u32at(b, q)? as usize;
        strings.push(q + 4..q + 4 + n);
        q += 4 + n;
    }
    let numbers = u32at(b, q)? as usize;
    need(numbers < 4096, "too many practice Lua numbers")?;
    q += 4 + 8 * numbers;
    let nested = u32at(b, q)? as usize;
    need(nested < 64, "too many practice Lua functions")?;
    q += 4;
    for _ in 0..nested {
        q = function(b, q, depth + 1)?.end;
    }
    let count = u32at(b, q)? as usize;
    q += 4;
    need(
        count < 100_000 && q + 4 * count <= b.len(),
        "unexpected practice Lua extent",
    )?;
    Ok(Function {
        strings,
        code: q..q + 4 * count,
        end: q + 4 * count,
    })
}

/// Rewrites the thresholds of the one graduation chunk, returning `(offset,
/// operand)` edits relative to `raw`, or nothing if this is another script.
fn thresholds(raw: &[u8]) -> Result<Vec<(usize, u32)>> {
    need(
        raw.get(..13) == Some(b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08"),
        "unexpected practice Lua header",
    )?;
    let f = function(raw, 21, 0)?;
    need(f.end == raw.len(), "unexpected practice Lua length")?;
    let has = |m: &[u8]| f.strings.iter().any(|s| raw[s.clone()] == *m);
    if !MARKERS.iter().all(|m| has(m)) {
        return Ok(Vec::new());
    }
    let word = |pc: usize| u32at(raw, f.code.start + 4 * pc);
    let mut found = Vec::new();
    for pc in 0..(f.code.len() / 4).saturating_sub(2) {
        let ins = word(pc)?;
        let name = f.strings.get((ins >> 6) as usize);
        if ins & 63 != PUSHSELF || name.is_none_or(|s| raw[s.clone()] != *PLAYERS) {
            continue;
        }
        let (call, operand) = (word(pc + 1)?, word(pc + 2)?);
        need(
            call & 63 == CALL && operand & 63 == PUSHINT,
            "unexpected practice party-size comparison",
        )?;
        found.push((f.code.start + 4 * (pc + 2), (operand >> 6) as i64 - BIAS));
    }
    need(
        found.iter().map(|x| x.1).eq(RETAIL),
        "unexpected practice party-size thresholds",
    )?;
    Ok(found
        .iter()
        .zip(PATCHED)
        .map(|(&(at, _), value)| (at, signed(PUSHINT, value)))
        .collect())
}

pub(super) fn expand(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let mut edits = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        if p.classes[p.objects[*index].class].name != "beLuaData" {
            continue;
        }
        let object = p.val(id as u32, 3)?;
        if !seen.insert(object) {
            continue;
        }
        p.class(object, "beLuaDataObject")?;
        let size = p.val(object, 2)? as usize;
        let range = p
            .memory
            .get(p.val(object, 3)? as usize)
            .and_then(|r| r.as_ref())
            .ok_or_else(|| bad("missing practice Lua memory"))?;
        need(size <= range.len(), "practice Lua exceeds memory block")?;
        let raw = &b[range.start..range.start + size];
        if !marked(raw) {
            continue;
        }
        edits.extend(thresholds(raw)?.into_iter().map(|(at, operand)| Edit {
            range: range.start + at..range.start + at + 4,
            bytes: operand.to_le_bytes().to_vec(),
        }));
    }
    edits.sort_by_key(|e| e.range.start);
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crown::river::lua::unsigned;

    // Test-owned bytecode with the same shape as the graduation chunk: one empty
    // nested function, the marker constants, and four party-size comparisons.
    fn chunk(values: [i64; 4]) -> Vec<u8> {
        fn string(b: &mut Vec<u8>, s: &[u8]) {
            b.extend((s.len() as u32).to_le_bytes());
            b.extend(s);
        }
        let mut b = b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08".to_vec();
        b.extend([0; 8]);
        string(&mut b, b"=(none)\0");
        b.extend([0; 4 + 4 + 1 + 4 + 4 + 4]);
        let strings: [&[u8]; 5] = [b"be\0", PLAYERS, MARKERS[0], MARKERS[1], MARKERS[2]];
        b.extend((strings.len() as u32).to_le_bytes());
        for s in strings {
            string(&mut b, s);
        }
        b.extend(0u32.to_le_bytes()); // numbers
        b.extend(0u32.to_le_bytes()); // nested functions
        let mut code = Vec::new();
        for v in values {
            code.extend([unsigned(12, 0), unsigned(16, 1)]);
            code.extend([2 | (1 << 15) | (1 << 6), signed(PUSHINT, v), 0]);
        }
        code.push(0);
        b.extend((code.len() as u32).to_le_bytes());
        for v in code {
            b.extend(v.to_le_bytes());
        }
        b
    }

    #[test]
    fn retail_thresholds_become_four_player_branch() {
        let raw = chunk([2, 3, 2, 3]);
        let edits = thresholds(&raw).unwrap();
        assert_eq!(edits.len(), 4);
        for ((at, operand), value) in edits.into_iter().zip(PATCHED) {
            assert_eq!(u32at(&raw, at).unwrap() & 63, PUSHINT);
            assert_eq!(operand, signed(PUSHINT, value));
        }
    }

    #[test]
    fn other_or_already_patched_scripts_are_not_guessed_at() {
        assert!(thresholds(&chunk([0, 1, 0, 1])).is_err());
        assert!(thresholds(&chunk([2, 2, 2, 2])).is_err());
    }
}
