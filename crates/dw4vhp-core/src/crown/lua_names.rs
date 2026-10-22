//! Experimental: rename whole string constants inside stage Lua 4.0.1 chunks, so scripts that
//! wait on or erase an enemy by model name follow the model a generator now spawns.
//! Only the constant table's length-prefixed strings change; no instruction, jump or stack
//! size is touched.
use super::*;

/// Lua 4.0.1 chunk signature, format flags and the test number `3.14159265358979323846e8`.
fn header() -> Vec<u8> {
    let mut h = b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08".to_vec();
    #[allow(clippy::excessive_precision)]
    h.extend(3.14159265358979323846e8_f64.to_le_bytes());
    h
}

/// A string constant: where its length prefix sits, and its text without the NUL.
pub(super) struct Constant {
    at: usize,
    end: usize,
    depth: usize,
    pub(super) text: Vec<u8>,
}

struct Reader<'a> {
    b: &'a [u8],
    q: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> Result<usize> {
        let v = u32at(self.b, self.q)? as usize;
        self.q += 4;
        Ok(v)
    }
    fn skip(&mut self, n: usize) -> Result<()> {
        need(self.q + n <= self.b.len(), "truncated Lua chunk")?;
        self.q += n;
        Ok(())
    }
    /// A length-prefixed string; `None` for the NULL string.
    fn string(&mut self) -> Result<Option<(usize, usize)>> {
        let at = self.q;
        let n = self.u32()?;
        self.skip(n)?;
        Ok((n > 0).then_some((at, self.q)))
    }
}

fn function(r: &mut Reader, depth: usize, out: &mut Vec<Constant>) -> Result<()> {
    need(depth < 32, "Lua nesting too deep")?;
    r.string()?;
    r.skip(8)?; // line, parameter count
    r.skip(1)?; // vararg
    r.skip(4)?; // max stack
    for _ in 0..r.u32()? {
        r.string()?;
        r.skip(8)?;
    }
    let lines = r.u32()?;
    r.skip(4 * lines)?;
    for _ in 0..r.u32()? {
        let (at, end) = r.string()?.ok_or_else(|| bad("NULL Lua string constant"))?;
        let text = r.b[at + 4..end]
            .strip_suffix(&[0])
            .unwrap_or(&r.b[at + 4..end]);
        out.push(Constant {
            at,
            end,
            depth,
            text: text.to_vec(),
        });
    }
    let numbers = r.u32()?;
    r.skip(8 * numbers)?;
    for _ in 0..r.u32()? {
        function(r, depth + 1, out)?;
    }
    let code = r.u32()?;
    r.skip(4 * code)
}

pub(super) fn constants(chunk: &[u8]) -> Result<Vec<Constant>> {
    let header = header();
    need(chunk.starts_with(&header), "not a Lua 4.0.1 chunk")?;
    let mut r = Reader {
        b: chunk,
        q: header.len(),
    };
    let mut out = Vec::new();
    function(&mut r, 0, &mut out)?;
    need(r.q == chunk.len(), "unexpected Lua chunk extent")?;
    Ok(out)
}

/// Replace every whole-string constant named in `map`. All replacements are made at once, so
/// swaps work. Returns `None` when nothing matched. A match inside a nested function is an
/// error rather than a silent miss.
pub(super) fn rename(chunk: &[u8], map: &BTreeMap<String, String>) -> Result<Option<Vec<u8>>> {
    let hits = constants(chunk)?
        .into_iter()
        .filter(|c| map.contains_key(&*String::from_utf8_lossy(&c.text)))
        .collect::<Vec<_>>();
    need(
        hits.iter().all(|c| c.depth == 0),
        "model name constant inside a nested Lua function",
    )?;
    if hits.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(chunk.len() + 16);
    let mut q = 0;
    for c in &hits {
        let new = &map[&*String::from_utf8_lossy(&c.text)];
        out.extend(&chunk[q..c.at]);
        out.extend((new.len() as u32 + 1).to_le_bytes());
        out.extend(new.as_bytes());
        out.push(0);
        q = c.end;
    }
    out.extend(&chunk[q..]);
    Ok(Some(out))
}

/// Rewrite the model-name constants of the named `beLuaData` scripts in a stage model pack.
/// Every script in `scripts` must contain every constant named in `map`.
pub(super) fn rename_in_pack(
    b: &[u8],
    scripts: &[String],
    map: &BTreeMap<String, String>,
) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let mut builder = Builder::new(&p);
    let mut done = BTreeMap::new();
    let mut template = None;
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let r = &p.objects[*index];
        if p.classes[r.class].name != "beLuaData" {
            continue;
        }
        let Some(script) = scripts.iter().find(|s| r.named(b, s)) else {
            continue;
        };
        let object = p.val(id as u32, 3)?;
        p.class(object, "beLuaDataObject")?;
        let sharers = (0..p.ids.len() as u32)
            .filter(|&i| {
                p.ids[i as usize].is_some_and(|x| {
                    p.classes[p.objects[x].class].name == "beLuaData"
                        && p.val(i, 3).ok() == Some(object)
                })
            })
            .count();
        need(
            sharers == 1,
            &format!("Lua script {script} shares its data object"),
        )?;
        let mem = p.val(object, 3)?;
        let size = p.val(object, 2)? as usize;
        let range = p
            .memory
            .get(mem as usize)
            .and_then(|r| r.as_ref())
            .ok_or_else(|| bad("missing Lua memory"))?;
        need(
            size == range.len(),
            "Lua data size differs from its memory block",
        )?;
        let raw = &b[range.start..range.start + size];
        let found = constants(raw)?;
        for name in map.keys() {
            need(
                found.iter().any(|c| c.text == name.as_bytes()),
                &format!("{script} has no constant {name}"),
            )?;
        }
        let out = rename(raw, map)?.ok_or_else(|| bad(&format!("no model name in {script}")))?;
        let length = out.len();
        let newmem = builder.mem(out, mem)?;
        builder.modify(object, &[(2, length as u32), (3, newmem)])?;
        done.insert(script.clone(), object);
        template = Some(mem);
    }
    need(done.len() == scripts.len(), "a named Lua script is missing")?;
    let edits = builder.finish(template.ok_or_else(|| bad("no Lua scripts named"))?)?;
    let out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        p.mapping == v.mapping[..p.mapping.len()]
            && b[p.pool..] == out[v.pool..v.pool + b.len() - p.pool],
        "Lua rename changed original IDs or memory",
    )?;
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(s: &str) -> Vec<u8> {
        let mut v = (s.len() as u32 + 1).to_le_bytes().to_vec();
        v.extend(s.as_bytes());
        v.push(0);
        v
    }

    fn chunk(constants: &[&str]) -> Vec<u8> {
        let mut c = header();
        c.extend(string("@x.lua"));
        c.extend([0u8; 8]);
        c.push(0);
        c.extend(2u32.to_le_bytes());
        c.extend(0u32.to_le_bytes()); // locals
        c.extend(0u32.to_le_bytes()); // lines
        c.extend((constants.len() as u32).to_le_bytes());
        for k in constants {
            c.extend(string(k));
        }
        c.extend(0u32.to_le_bytes()); // numbers
        c.extend(0u32.to_le_bytes()); // functions
        c.extend(1u32.to_le_bytes());
        c.extend(0u32.to_le_bytes());
        c
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn renames_whole_constants_and_swaps() {
        let old = chunk(&["MODELCTRL", "e_ogre", "e_mummy", "e_ogre_x"]);
        let new = rename(&old, &map(&[("e_ogre", "e_mummy"), ("e_mummy", "e_ogre")]))
            .unwrap()
            .unwrap();
        assert_eq!(new, chunk(&["MODELCTRL", "e_mummy", "e_ogre", "e_ogre_x"]));
    }

    #[test]
    fn grows_and_shrinks() {
        let old = chunk(&["e_ogre"]);
        let new = rename(&old, &map(&[("e_ogre", "e_infermo")]))
            .unwrap()
            .unwrap();
        assert_eq!(new, chunk(&["e_infermo"]));
        assert_eq!(new.len(), old.len() + 3);
        assert_eq!(rename(&old, &map(&[("e_goburi", "e_x")])).unwrap(), None);
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut old = chunk(&["e_ogre"]);
        old.push(0);
        assert!(rename(&old, &map(&[("e_ogre", "e_x")])).is_err());
    }
}
