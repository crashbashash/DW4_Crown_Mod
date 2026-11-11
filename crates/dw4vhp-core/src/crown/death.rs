//! Vanilla death softlock. Every stage's death script plays the first-death
//! tutorial while flag 508 is set: it opens the `i_messwnsp` window and waits
//! for it. Only the early stages register that window, and retail clears 508
//! on the first death there or when Blossomon or Mammothmon falls. A player
//! who skips both (for example by entering Goblin Fortress early) waits
//! forever on a window that never opens, in that stage and every later one.
//! In stages without the window, take the ordinary game-over path instead.
use super::river::lua::signed;
use super::*;

const FLAG: i64 = 508;
const WINDOW: &str = "i_messwnsp";

/// `None` when the stage has no death tutorial or can show it.
pub(super) fn fix(pack: &[u8]) -> Result<Option<Vec<u8>>> {
    if memchr::memmem::find(pack, b"SQ_GAMEOVER\0").is_none()
        || memchr::memmem::find(pack, format!("{WINDOW}\0").as_bytes()).is_none()
    {
        return Ok(None);
    }
    let p = Pack::parse(pack)?;
    if graft::find_named(&p, "beModelCtrlInfo", WINDOW)?.is_some() {
        return Ok(None);
    }
    let mut out = pack.to_vec();
    let mut changed = 0;
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let object = p.val(id, 3)?;
        let range = p
            .memory
            .get(p.val(object, 3)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing Lua memory"))?;
        let found = lua_names::constants(&pack[range.clone()])?;
        if !found.iter().any(|c| c.text == WINDOW.as_bytes())
            || !found.iter().any(|c| c.text == b"SQ_GAMEOVER")
        {
            continue;
        }
        let sites = tutorial_branches(&pack[range.clone()], 1)?;
        need(
            sites.len() <= 1,
            "more than one first-death tutorial branch",
        )?;
        for at in sites {
            // Same-size, in-place instruction edit: no memory moves.
            let at = range.start + at;
            out[at..at + 4].copy_from_slice(&signed(6, 2).to_le_bytes());
            changed += 1;
        }
    }
    Ok((changed > 0).then_some(out))
}

/// Byte offsets of `value` in `be:FLGisOn(508) == value`, in any function of
/// the chunk (pit-fall death scripts define local functions). Retail compares
/// with 1; FLGisOn returns 0 or 1, so comparing with 2 never takes the branch.
fn tutorial_branches(chunk: &[u8], value: i64) -> Result<Vec<usize>> {
    fn string(b: &[u8], q: &mut usize) -> Result<Vec<u8>> {
        let n = u32at(b, *q)? as usize;
        let s = b
            .get(*q + 4..*q + 4 + n)
            .ok_or_else(|| bad("truncated Lua string"))?;
        *q += 4 + n;
        Ok(s.strip_suffix(&[0]).unwrap_or(s).to_vec())
    }
    fn function(
        b: &[u8],
        q: &mut usize,
        depth: usize,
        value: i64,
        out: &mut Vec<usize>,
    ) -> Result<()> {
        need(depth < 32, "Lua nesting too deep")?;
        string(b, q)?;
        *q += 4 + 4 + 1 + 4; // line, parameters, vararg, max stack
        let locals = u32at(b, *q)?;
        *q += 4;
        for _ in 0..locals {
            string(b, q)?;
            *q += 8; // start and end pc
        }
        let lines = u32at(b, *q)? as usize;
        *q += 4 + 4 * lines;
        let count = u32at(b, *q)? as usize;
        *q += 4;
        let strings = (0..count)
            .map(|_| string(b, q))
            .collect::<Result<Vec<_>>>()?;
        let numbers = u32at(b, *q)? as usize;
        *q += 4 + 8 * numbers;
        let nested = u32at(b, *q)?;
        *q += 4;
        for _ in 0..nested {
            function(b, q, depth + 1, value, out)?;
        }
        let count = u32at(b, *q)? as usize;
        let code = *q + 4;
        *q = code + 4 * count;
        need(*q <= b.len(), "truncated Lua code")?;
        let word = |pc: usize| u32at(b, code + 4 * pc);
        for pc in 0..count.saturating_sub(4) {
            let call = word(pc)?;
            if call & 63 == 16
                && strings.get((call >> 6) as usize).map(Vec::as_slice) == Some(b"FLGisOn")
                && word(pc + 1)? == signed(6, FLAG)
                && word(pc + 2)? & 63 == 2
                && word(pc + 3)? == signed(6, value)
                && word(pc + 4)? & 63 == 32
            {
                out.push(code + 4 * (pc + 3));
            }
        }
        Ok(())
    }
    // Signature and format bytes, then the test number.
    need(chunk.starts_with(b"\x1bLua@"), "not a Lua 4.0.1 chunk")?;
    let mut q = 21;
    let mut out = Vec::new();
    function(chunk, &mut q, 0, value, &mut out)?;
    need(q == chunk.len(), "unexpected Lua chunk extent")?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stages that register the tutorial window keep it; every other stage
    /// with the shared death script loses only the tutorial branch.
    #[test]
    fn retail_death_tutorial_runs_only_where_its_window_exists() {
        for variable in [
            "DW4_RETAIL_USA_ISO",
            "DW4_RETAIL_EUROPE_ISO",
            "DW4_RETAIL_JAPAN_ISO",
            "DW4_RETAIL_GC_USA_ISO",
            "DW4_RETAIL_GC_JAPAN_ISO",
        ] {
            let Some(path) = std::env::var_os(variable) else {
                continue;
            };
            let file = std::fs::File::open(path).unwrap();
            let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let (base, size) = if crate::release::detect(&iso).unwrap().is_gamecube() {
                let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + size];
            let (mut fixed, mut kept) = (Vec::new(), Vec::new());
            for i in 0..u32at(afs, 4).unwrap() as usize {
                let off = u32at(afs, 8 + 8 * i).unwrap() as usize;
                let len = u32at(afs, 12 + 8 * i).unwrap() as usize;
                let data = &afs[off..off + len];
                if memchr::memmem::find(data, b"SQ_GAMEOVER\0").is_none()
                    || memchr::memmem::find(data, b"i_messwnsp\0").is_none()
                {
                    continue;
                }
                match fix(data).unwrap() {
                    Some(out) => {
                        // The rewritten stage has no tutorial branch left.
                        assert!(fix(&out).unwrap().is_none(), "{variable} entry {i}");
                        fixed.push(i);
                    }
                    None => kept.push(i),
                }
            }
            // The four Goblin Fortress rooms are where the softlock was reported.
            for room in [772, 779, 786, 793] {
                assert!(fixed.contains(&room), "{variable}: {room}");
            }
            for tutorial in [179, 614] {
                assert!(kept.contains(&tutorial), "{variable}: {tutorial}");
            }
            println!("{variable}: fixed {} stages, kept {kept:?}", fixed.len());
        }
    }
}
