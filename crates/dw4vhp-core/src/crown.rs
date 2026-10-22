//! Expands the authored crown resources together: model and animation RAM,
//! private mutable scene roots, and a private two-word visibility mask.
//! No executable offsets or external game metadata are used. Original geometry,
//! original IDs and pool contents remain intact. Archive order is preserved.
//! The river model pack also receives missile damage and turret crown scripts.
use crate::{
    error::{Error, Result},
    gcm, iso9660,
    region::WriteRegion,
};
use std::{collections::BTreeMap, io::Write, ops::Range};

mod drops;
pub mod graft;
mod lua_names;
mod practice;
mod random;
pub use random::Options as RandomOptions;
pub use random::{DONORS as RANDOM_DONORS, RULES_VERSION as RANDOM_RULES_VERSION};
mod river;
mod spawns;

pub const CAPACITY: u32 = 64;
const NULL: u32 = u32::MAX;
fn bad(s: &str) -> Error {
    Error::CrownData(s.into())
}
fn need(ok: bool, s: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(bad(s))
    }
}
fn u32at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4)
            .ok_or_else(|| bad("truncated word"))?
            .try_into()
            .unwrap(),
    ))
}
fn u16at(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2)
            .ok_or_else(|| bad("truncated descriptor"))?
            .try_into()
            .unwrap(),
    ))
}
fn put(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn a4(n: usize) -> usize {
    (n + 3) & !3
}
fn a2048(n: usize) -> usize {
    (n + 2047) & !2047
}
#[derive(Clone)]
struct Class {
    name: String,
    fields: Vec<(u16, u16, u16)>,
}
#[derive(Clone)]
struct Record {
    class: usize,
    range: Range<usize>,
    fields: BTreeMap<u16, Range<usize>>,
}
impl Record {
    fn field(&self, n: u16) -> Result<Range<usize>> {
        self.fields
            .get(&n)
            .cloned()
            .ok_or_else(|| bad("missing field"))
    }
    fn val(&self, b: &[u8], n: u16) -> Result<u32> {
        let r = self.field(n)?;
        need(r.len() == 4, "unexpected field width")?;
        u32at(b, r.start)
    }
    fn named(&self, b: &[u8], name: &str) -> bool {
        self.field(2)
            .ok()
            .and_then(|r| b.get(r.start + 4..r.end))
            .is_some_and(|s| s.split(|c| *c == 0).next() == Some(name.as_bytes()))
    }
    fn patch(&self, b: &[u8], changes: &[(u16, u32)]) -> Result<Vec<u8>> {
        let mut out = b[self.range.clone()].to_vec();
        for &(n, v) in changes {
            let r = self.field(n)?;
            need(r.len() == 4, "unexpected patch width")?;
            put(&mut out, r.start - self.range.start, v);
        }
        Ok(out)
    }
}
fn ident(b: &[u8], p: usize) -> Option<&str> {
    let rest = b.get(p..)?;
    let len = rest.iter().position(|c| *c == 0)?;
    if !(3..=61).contains(&len)
        || !(rest[0].is_ascii_alphabetic() || rest[0] == b'_')
        || !rest[..len]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b':')
    {
        return None;
    }
    std::str::from_utf8(&rest[..len]).ok()
}
fn classes(b: &[u8]) -> Result<Vec<Class>> {
    class_table(b).map(|(classes, _, _)| classes)
}
/// The class table, and the offsets where it starts and ends.
fn class_table(b: &[u8]) -> Result<(Vec<Class>, usize, usize)> {
    let mut best = Vec::new();
    let (mut best_start, mut best_end) = (0, 0);
    for p in memchr::memmem::find_iter(&b[..b.len().min(0x8000)], b"\0igObject\0") {
        let start = p + 1;
        let limit = b.len().min(start + 0x8000);
        let mut cur = start;
        let mut out = Vec::new();
        let mut end_of_table = 0;
        while let Some(name) = ident(b, cur) {
            let pad = (cur + name.len() + 2) & !1;
            let mut fields = Vec::new();
            let mut next = None;
            for n in 0..600 {
                let q = pad + 6 * n;
                if q > limit {
                    break;
                }
                if q > cur && ident(b, q).is_some() {
                    next = Some(q);
                    break;
                }
                if q + 6 > b.len() {
                    break;
                }
                let (t, o, s) = (u16at(b, q)?, u16at(b, q + 2)?, u16at(b, q + 4)?);
                if t >= 48 || !(1..4096).contains(&o) {
                    break;
                }
                fields.push((t, o, s));
            }
            if next.is_none() && fields.is_empty() {
                break;
            }
            let end = pad + 6 * fields.len();
            out.push(Class {
                name: name.into(),
                fields,
            });
            match next {
                Some(q) => cur = q,
                None => {
                    end_of_table = end;
                    break;
                }
            }
        }
        if out.len() > best.len() {
            best = out;
            best_start = start;
            best_end = end_of_table;
        }
    }
    need(
        best.len() == u32at(b, 12)? as usize,
        "type directory count mismatch",
    )?;
    Ok((best, best_start, best_end))
}
/// A class definition with the hierarchy facts the pack keeps in a table of its own: one
/// 24-byte record per class, `[name size, 1, 0, own fields, parent class, field slots]`.
#[derive(Clone)]
struct ClassDef {
    class: Class,
    parent: Option<String>,
    slots: u32,
}
/// The hierarchy record of every class of the pack.
fn class_defs(b: &[u8]) -> Result<Vec<ClassDef>> {
    let (classes, start, _) = class_table(b)?;
    let table = start
        .checked_sub(24 * classes.len())
        .ok_or_else(|| bad("class records before the file start"))?;
    let mut out = Vec::new();
    for (i, class) in classes.iter().enumerate() {
        let at = table + 24 * i;
        need(
            u32at(b, at + 4)? == 1
                && u32at(b, at + 8)? == 0
                && u32at(b, at + 12)? as usize == class.fields.len(),
            "class record does not match its class",
        )?;
        let parent = u32at(b, at + 16)?;
        out.push(ClassDef {
            class: class.clone(),
            parent: (parent != u32::MAX)
                .then(|| classes.get(parent as usize).map(|c| c.name.clone()))
                .map(|c| c.ok_or_else(|| bad("class parent out of range")))
                .transpose()?,
            slots: u32at(b, at + 20)?,
        });
    }
    Ok(out)
}
/// Append classes (parents before children) to a pack, so it can hold objects of those classes.
fn add_classes(b: &[u8], add: &[ClassDef]) -> Result<Vec<u8>> {
    let (classes, start, end) = class_table(b)?;
    need(end > start && !add.is_empty(), "no class table end")?;
    let k = add.len();
    let mut names: Vec<&str> = classes.iter().map(|c| c.name.as_str()).collect();
    let (mut records, mut entries) = (Vec::new(), Vec::new());
    // Records go in front of the first class entry, so the entries move 24 bytes per class.
    let mut at = end + 24 * k;
    for d in add {
        need(
            !names.contains(&d.class.name.as_str()),
            "class already defined",
        )?;
        let parent = match &d.parent {
            Some(p) => names
                .iter()
                .position(|n| n == p)
                .ok_or_else(|| bad("class parent is not defined"))? as u32,
            None => u32::MAX,
        };
        let pad = (at + d.class.name.len() + 2) & !1;
        for v in [
            (pad - at) as u32,
            1,
            0,
            d.class.fields.len() as u32,
            parent,
            d.slots,
        ] {
            records.extend(v.to_le_bytes());
        }
        entries.extend(d.class.name.as_bytes());
        entries.resize(entries.len() + pad - at - d.class.name.len(), 0);
        for &(t, o, s) in &d.class.fields {
            entries.extend(t.to_le_bytes());
            entries.extend(o.to_le_bytes());
            entries.extend(s.to_le_bytes());
        }
        at = end + 24 * k + entries.len();
        names.push(&d.class.name);
    }
    let added = records.len() + entries.len();
    let mut out = b[..start].to_vec();
    out.extend(records);
    out.extend(&b[start..end]);
    out.extend(entries);
    out.extend(&b[end..]);
    put(&mut out, 12, (classes.len() + k) as u32);
    // Header word 2 is the byte size of the type directory: the class records and entries.
    put(&mut out, 8, u32at(b, 8)? + added as u32);
    Ok(out)
}
fn record(b: &[u8], p: usize, classes: &[Class]) -> Result<Record> {
    let class = u32at(b, p)? as usize;
    let size = u32at(b, p + 4)? as usize;
    let c = classes
        .get(class)
        .ok_or_else(|| bad("invalid record class"))?;
    need(
        size >= 8 && p.checked_add(size).is_some_and(|e| e <= b.len()),
        "invalid record extent",
    )?;
    let mut q = p + 8;
    let mut fields = BTreeMap::new();
    for &(t, n, s) in &c.fields {
        let width = if t == 4 {
            let len = u32at(b, q)? as usize;
            need(len <= 8192 && len.is_multiple_of(4), "invalid string size")?;
            4 + len
        } else {
            a4(s as usize)
        };
        need(q + width <= p + size, "field outside record")?;
        need(fields.insert(n, q..q + width).is_none(), "duplicate field")?;
        q += width;
    }
    need(q == p + size, "record size disagrees with descriptors")?;
    Ok(Record {
        class,
        range: p..q,
        fields,
    })
}
struct Pack<'a> {
    b: &'a [u8],
    classes: Vec<Class>,
    dirs: Vec<Record>,
    objects: Vec<Record>,
    ids: Vec<Option<usize>>,
    memory: Vec<Option<Range<usize>>>,
    mapping: Vec<u16>,
    table: usize,
    pool: usize,
}
impl<'a> Pack<'a> {
    fn parse(b: &'a [u8]) -> Result<Self> {
        need(b.len() >= 48, "truncated pack header")?;
        let classes = classes(b)?;
        let count = u32at(b, 4)? as usize;
        need(
            count > 0 && count <= u16::MAX as usize,
            "invalid directory count",
        )?;
        let mut dirs = None;
        for start in 0x800..b.len().min(0x20000) {
            let Ok(ci) = u32at(b, start) else { break };
            let Some(c) = classes.get(ci as usize) else {
                continue;
            };
            if !c.name.ends_with("Entry") {
                continue;
            }
            let mut q = start;
            let mut candidate = Vec::new();
            for _ in 0..count {
                match record(b, q, &classes) {
                    Ok(r) if classes[r.class].name.ends_with("Entry") => {
                        q = r.range.end;
                        candidate.push(r)
                    }
                    _ => break,
                }
            }
            if candidate.len() == count && q - start == u32at(b, 0)? as usize {
                dirs = Some(candidate);
                break;
            }
        }
        let dirs = dirs.ok_or_else(|| bad("cannot locate directory section"))?;
        let table = dirs.last().unwrap().range.end;
        let tb = u32at(b, table)? as usize;
        let n = u32at(b, table + 4)? as usize;
        need(
            n > 0 && n < 1_000_000 && tb == 8 + 2 * n && table + tb + 4 <= b.len(),
            "invalid ID table",
        )?;
        need((u32at(b, table + tb)? as usize) < n, "invalid root ID")?;
        let mapping = (0..n)
            .map(|i| u16at(b, table + 8 + 2 * i))
            .collect::<Result<Vec<_>>>()?;
        let objects_start = table + tb + 4;
        let mut q = objects_start;
        let mut objects = Vec::new();
        let obj_count = u32at(b, 20)? as usize;
        need(obj_count <= b.len() / 8, "invalid object count")?;
        for _ in 0..obj_count {
            let r = record(b, q, &classes)?;
            q = r.range.end;
            objects.push(r);
        }
        need(
            q - objects_start == u32at(b, 16)? as usize && b.len() - q == u32at(b, 24)? as usize,
            "object/pool byte counts disagree",
        )?;
        let pool = q;
        let mut ids = vec![None; n];
        let mut memory = vec![None; n];
        let mut obj = 0;
        for (i, &idx) in mapping.iter().enumerate() {
            let d = dirs
                .get(idx as usize)
                .ok_or_else(|| bad("directory ID out of bounds"))?;
            match classes[d.class].name.as_str() {
                "igObjectDirEntry" => {
                    let r = objects
                        .get(obj)
                        .ok_or_else(|| bad("object ID out of bounds"))?;
                    need(
                        d.val(b, 11)? as usize == r.class,
                        "object remap type mismatch",
                    )?;
                    ids[i] = Some(obj);
                    obj += 1;
                }
                "igMemoryDirEntry" => {
                    let size = d.val(b, 7)? as usize;
                    need(size <= b.len() - q, "memory block outside pool")?;
                    memory[i] = Some(q..q + size);
                    q += a4(size);
                    need(q <= b.len(), "memory alignment outside pool")?;
                }
                _ => {}
            }
        }
        need(
            obj == objects.len() && q == b.len(),
            "remap/pool extent mismatch",
        )?;
        Ok(Self {
            b,
            classes,
            dirs,
            objects,
            ids,
            memory,
            mapping,
            table,
            pool,
        })
    }
    fn rec(&self, id: u32) -> Result<&Record> {
        let idx = self
            .ids
            .get(id as usize)
            .and_then(|x| *x)
            .ok_or_else(|| bad("expected object ID"))?;
        Ok(&self.objects[idx])
    }
    fn class(&self, id: u32, name: &str) -> Result<()> {
        need(
            self.classes[self.rec(id)?.class].name == name,
            "unexpected crown resource class",
        )
    }
    fn val(&self, id: u32, n: u16) -> Result<u32> {
        self.rec(id)?.val(self.b, n)
    }
    fn array(&self, id: u32) -> Result<Vec<u32>> {
        let count = self.val(id, 2)? as usize;
        need(
            count <= self.val(id, 3)? as usize,
            "list count exceeds capacity",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mem = self.val(id, 4)?;
        let r = self
            .memory
            .get(mem as usize)
            .and_then(|x| x.as_ref())
            .ok_or_else(|| bad("expected list memory"))?;
        need(count <= r.len() / 4, "list exceeds memory block")?;
        (0..count).map(|i| u32at(self.b, r.start + 4 * i)).collect()
    }
    fn named(&self, class: &str, name: &str) -> Result<u32> {
        let ids = self
            .ids
            .iter()
            .enumerate()
            .filter_map(|(id, idx)| idx.map(|i| (id, &self.objects[i])))
            .filter(|(_, r)| self.classes[r.class].name == class && r.named(self.b, name))
            .map(|(id, _)| id as u32)
            .collect::<Vec<_>>();
        need(ids.len() == 1, "expected one crown controller")?;
        Ok(ids[0])
    }
}
#[derive(Debug)]
struct Edit {
    range: Range<usize>,
    bytes: Vec<u8>,
}
struct Builder<'p, 'a> {
    p: &'p Pack<'a>,
    dirs: Vec<u8>,
    objects: Vec<u8>,
    pool: Vec<u8>,
    mapping: Vec<u16>,
    object_count: u32,
    edits: Vec<Edit>,
    /// Bytes inserted at the end of existing memory blocks: (offset in the pool, bytes).
    grows: Vec<(usize, Vec<u8>)>,
}
impl<'p, 'a> Builder<'p, 'a> {
    fn new(p: &'p Pack<'a>) -> Self {
        Self {
            p,
            dirs: vec![],
            objects: vec![],
            pool: vec![],
            mapping: vec![],
            object_count: 0,
            edits: vec![],
            grows: vec![],
        }
    }
    /// Where blocks were grown, as (offset in the pool, length), in pool order.
    fn grown(&self) -> Vec<(usize, usize)> {
        let mut g = self
            .grows
            .iter()
            .map(|(at, e)| (*at, e.len()))
            .collect::<Vec<_>>();
        g.sort_unstable();
        g
    }
    /// Append `extra` to an existing memory block in place, keeping its ID, so the object
    /// that owns it keeps the same block (the engine never sees a replacement).
    fn grow_mem(&mut self, block: u32, extra: Vec<u8>) -> Result<()> {
        need(extra.len().is_multiple_of(4), "unaligned block growth")?;
        let range = self
            .p
            .memory
            .get(block as usize)
            .and_then(|r| r.clone())
            .ok_or_else(|| bad("missing block to grow"))?;
        let idx = *self
            .p
            .mapping
            .get(block as usize)
            .ok_or_else(|| bad("missing block entry"))? as usize;
        let dir = &self.p.dirs[idx];
        need(
            self.p.classes[dir.class].name == "igMemoryDirEntry",
            "wrong memory directory class",
        )?;
        let size = dir.val(self.p.b, 7)? as usize;
        need(
            size == range.len(),
            "block size differs from its directory entry",
        )?;
        self.edits.push(Edit {
            range: dir.range.clone(),
            bytes: dir.patch(self.p.b, &[(7, (size + extra.len()) as u32)])?,
        });
        self.grows.push((range.end - self.p.pool, extra));
        Ok(())
    }
    fn add(&mut self, dir: Vec<u8>, payload: Vec<u8>, memory: bool) -> Result<u32> {
        let id = self.p.ids.len() + self.mapping.len();
        let idx = self.p.dirs.len() + self.mapping.len();
        need(
            idx <= u16::MAX as usize,
            "expanded directory exceeds 16-bit index",
        )?;
        self.mapping.push(idx as u16);
        self.dirs.extend(dir);
        if memory {
            self.pool.extend(&payload);
            self.pool.resize(a4(self.pool.len()), 0);
        } else {
            self.objects.extend(payload);
            self.object_count += 1;
        }
        Ok(id as u32)
    }
    fn obj(&mut self, id: u32, changes: &[(u16, u32)]) -> Result<u32> {
        let raw = self.p.rec(id)?.patch(self.p.b, changes)?;
        self.obj_data(id, raw)
    }
    fn obj_data(&mut self, id: u32, raw: Vec<u8>) -> Result<u32> {
        let r = self.p.rec(id)?;
        let directory = self
            .p
            .dirs
            .iter()
            .find(|d| {
                self.p.classes[d.class].name == "igObjectDirEntry"
                    && d.val(self.p.b, 11).ok() == Some(r.class as u32)
            })
            .ok_or_else(|| bad("missing object directory template"))?;
        self.add(self.p.b[directory.range.clone()].to_vec(), raw, false)
    }
    fn mem(&mut self, bytes: Vec<u8>, template: u32) -> Result<u32> {
        let idx = *self
            .p
            .mapping
            .get(template as usize)
            .ok_or_else(|| bad("missing memory template"))? as usize;
        let d = &self.p.dirs[idx];
        need(
            self.p.classes[d.class].name == "igMemoryDirEntry",
            "wrong memory template class",
        )?;
        self.add(d.patch(self.p.b, &[(7, bytes.len() as u32)])?, bytes, true)
    }
    fn modify(&mut self, id: u32, changes: &[(u16, u32)]) -> Result<()> {
        let r = self.p.rec(id)?;
        self.edits.push(Edit {
            range: r.range.clone(),
            bytes: r.patch(self.p.b, changes)?,
        });
        Ok(())
    }
    fn finish(mut self, template: u32) -> Result<Vec<Edit>> {
        if !self.mapping.len().is_multiple_of(2) {
            self.mem(vec![0; 4], template)?;
        }
        let n = (self.p.ids.len() + self.mapping.len()) as u32;
        let mut table = self.dirs.clone();
        table.extend((8 + 2 * n).to_le_bytes());
        table.extend(n.to_le_bytes());
        table.extend(&self.p.b[self.p.table + 8..self.p.table + 8 + 2 * self.p.ids.len()]);
        for &v in &self.mapping {
            table.extend(v.to_le_bytes());
        }
        self.edits.push(Edit {
            range: self.p.table..self.p.table + 8 + 2 * self.p.ids.len(),
            bytes: table,
        });
        let object_bytes = self.objects.len();
        let mut objects = self.objects;
        let mut pool = self.p.b[self.p.pool..].to_vec();
        let mut grown = 0;
        self.grows.sort_by_key(|g| std::cmp::Reverse(g.0));
        for (at, extra) in &self.grows {
            grown += extra.len();
            pool.splice(*at..*at, extra.iter().copied());
        }
        objects.extend(pool);
        objects.extend(&self.pool);
        self.edits.push(Edit {
            range: self.p.pool..self.p.b.len(),
            bytes: objects,
        });
        // The original pool is shared as a source segment below rather than retained.
        for (word, add) in [
            (0, self.dirs.len()),
            (1, self.mapping.len()),
            (4, object_bytes),
            (5, self.object_count as usize),
            (6, self.pool.len() + grown),
        ] {
            self.edits.push(Edit {
                range: word * 4..word * 4 + 4,
                bytes: (u32at(self.p.b, word * 4)? + add as u32)
                    .to_le_bytes()
                    .to_vec(),
            });
        }
        self.edits.sort_by_key(|e| e.range.start);
        Ok(self.edits)
    }
}
fn words(ids: &[u32]) -> Vec<u8> {
    ids.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn materialize(b: &[u8], edits: &[Edit]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut q = 0;
    for e in edits {
        need(
            e.range.start >= q && e.range.end <= b.len(),
            "overlapping pack edits",
        )?;
        out.extend(&b[q..e.range.start]);
        out.extend(&e.bytes);
        q = e.range.end;
    }
    out.extend(&b[q..]);
    Ok(out)
}
fn expand(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let info = p.named("beModelCtrlInfo", "i_rareicon")?;
    let ramlist = p.val(info, 6)?;
    let mut rams = p.array(ramlist)?;
    need(rams.len() == 4, "original crown capacity must be four")?;
    let mut nodes = Vec::new();
    for &ram in &rams {
        p.class(ram, "beBaseInfoRam")?;
        let one = p.array(p.val(ram, 10)?)?;
        need(one.len() == 1, "expected one crown model datum")?;
        p.class(one[0], "beModelCtrlInfoRamData")?;
        nodes.push(p.val(one[0], 4)?);
    }
    let parents = p
        .ids
        .iter()
        .enumerate()
        .filter_map(|(id, x)| x.map(|i| (id, &p.objects[i])))
        .filter(|(id, r)| {
            p.classes[r.class].name == "igNodeList"
                && p.array(*id as u32).ok().as_ref() == Some(&nodes)
        })
        .map(|(id, _)| id as u32)
        .collect::<Vec<_>>();
    need(parents.len() == 1, "expected one crown scene parent")?;
    let parent = parents[0];
    let action = p.named("beAction2Info", "i_rareicon")?;
    let actionlist = p.val(action, 6)?;
    let mut actions = p.array(actionlist)?;
    need(actions.len() == 4, "expected four crown animation slots")?;
    for (a, n) in actions.iter().zip(&nodes) {
        p.class(*a, "beAction2InfoRam")?;
        need(p.val(*a, 11)? == *n, "crown model/animation mismatch")?;
    }
    let switch = p.val(action, 7)?;
    p.class(switch, "igSwitch")?;
    need(p.val(switch, 7)? == parent, "crown switch parent mismatch")?;
    let mask = p.val(switch, 8)?;
    p.class(mask, "igBitMask")?;
    need(
        p.val(mask, 5)? == 4 && p.val(mask, 2)? == 1,
        "unexpected crown visibility mask",
    )?;
    let mut enemy_slots = 0;
    for (id, x) in p.ids.iter().enumerate() {
        if let Some(idx) = x {
            let r = &p.objects[*idx];
            if p.classes[r.class].name == "beModelCtrlInfo"
                && r.field(7).ok().is_some_and(|range| {
                    b.get(range.start + 4..range.end)
                        .is_some_and(|s| s.starts_with(b"KIND_ENEMY\0"))
                })
            {
                enemy_slots += p.val(p.val(id as u32, 6)?, 2)?;
            }
        }
    }
    need(
        enemy_slots + 4 <= CAPACITY,
        "enemy pools exceed crown capacity",
    )?;
    let mut builder = Builder::new(&p);
    for n in 4..CAPACITY as usize {
        let ram = rams[n % 4];
        let dl = p.val(ram, 10)?;
        let one = p.array(dl)?[0];
        let node = p.val(one, 4)?;
        p.class(node, "beModelCtrlNode2")?;
        let child = p.val(node, 7)?;
        let cm = builder.mem(words(&p.array(child)?), p.val(child, 4)?)?;
        let childlist = builder.obj(child, &[(4, cm)])?;
        let attrs = p.val(node, 15)?;
        need(p.array(attrs)?.is_empty(), "nonempty crown overrides")?;
        let attrlist = builder.obj(attrs, &[])?;
        let newnode = builder.obj(node, &[(7, childlist), (15, attrlist)])?;
        let newone = builder.obj(one, &[(4, newnode), (5, NULL)])?;
        let dm = builder.mem(words(&[newone]), p.val(dl, 4)?)?;
        let datalist = builder.obj(dl, &[(4, dm)])?;
        let newram = builder.obj(ram, &[(10, datalist)])?;
        let ar = actions[n % 4];
        let ap = p.val(ar, 12)?;
        let actor = p.val(ar, 13)?;
        need(
            p.array(ap)?.is_empty() && p.array(actor)?.is_empty(),
            "nonempty crown animation template",
        )?;
        let newap = builder.obj(ap, &[])?;
        let newactor = builder.obj(actor, &[])?;
        let newaction = builder.obj(
            ar,
            &[(11, newnode), (12, newap), (13, newactor), (14, NULL)],
        )?;
        rams.push(newram);
        nodes.push(newnode);
        actions.push(newaction);
    }
    for (list, ids) in [(ramlist, &rams), (parent, &nodes), (actionlist, &actions)] {
        let mem = builder.mem(words(ids), p.val(list, 4)?)?;
        builder.modify(list, &[(2, CAPACITY), (3, CAPACITY), (4, mem)])?;
    }
    let mm = builder.mem(vec![0; 8], p.val(mask, 4)?)?;
    let newmask = builder.obj(mask, &[(2, 2), (3, 2), (4, mm), (5, CAPACITY)])?;
    builder.modify(switch, &[(8, newmask)])?;
    let edits = builder.finish(p.val(parent, 4)?)?;
    let out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        v.array(ramlist)? == rams
            && v.array(parent)? == nodes
            && v.array(actionlist)? == actions
            && v.val(v.val(switch, 8)?, 5)? == CAPACITY,
        "expanded crown graph failed verification",
    )?;
    for (a, n) in actions.iter().zip(nodes.iter()) {
        need(v.val(*a, 11)? == *n, "expanded animation/node mismatch")?;
    }
    need(
        b[p.pool..] == out[v.pool..v.pool + b.len() - p.pool],
        "original memory pool changed",
    )?;
    need(
        p.mapping == v.mapping[..p.mapping.len()],
        "original IDs changed",
    )?;
    for (id, old) in p.ids.iter().enumerate() {
        if let Some(idx) = old {
            let r = &p.objects[*idx];
            let new = v.rec(id as u32)?;
            if ![ramlist, parent, actionlist, switch].contains(&(id as u32)) {
                need(
                    b[r.range.clone()] == out[new.range.clone()],
                    "unrelated original object changed",
                )?;
            }
        }
    }
    let unique = nodes
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    need(unique.len() == CAPACITY as usize, "crown roots are shared")?;
    for (id, idx) in v.ids.iter().enumerate().skip(p.ids.len()) {
        if let Some(idx) = idx {
            let r = &v.objects[*idx];
            for &(ty, n, _) in &v.classes[r.class].fields {
                if ty == 0 || ty == 7 {
                    let value = r.val(&out, n)?;
                    if value != NULL {
                        need(
                            (value as usize) < v.ids.len(),
                            "new reference outside ID table",
                        )?;
                        need(
                            if ty == 0 {
                                v.ids[value as usize].is_some()
                            } else {
                                v.memory[value as usize].is_some()
                            },
                            "new reference has incorrect kind",
                        )?;
                    }
                }
            }
        } else {
            need(v.memory[id].is_some(), "unknown generated resource")?;
        }
    }
    // Split the insertion at the original pool: retain the large unchanged pool
    // through a source range, so all 85 patch plans use only a few MB of RAM.
    let mut compact = Vec::new();
    for e in edits {
        if e.range.start == p.pool && e.range.end == b.len() {
            let added = e.bytes.len() - (b.len() - p.pool);
            let objadd = (u32at(&out, 16)? - u32at(b, 16)?) as usize;
            need(added >= objadd, "bad insertion sizes")?;
            compact.push(Edit {
                range: p.pool..p.pool,
                bytes: e.bytes[..objadd].to_vec(),
            });
            compact.push(Edit {
                range: b.len()..b.len(),
                bytes: e.bytes[objadd + b.len() - p.pool..].to_vec(),
            });
        } else {
            compact.push(e);
        }
    }
    Ok(compact)
}

#[derive(Debug)]
pub(crate) enum Piece {
    Source(Range<usize>),
    Bytes(Vec<u8>),
}
/// A small, self-contained archive rebuild recipe; original bulk data stays mmap-backed.
pub struct CrownPlan {
    pieces: Vec<Piece>,
    /// What the enemy randomiser did, when it ran: recorded in the image signature.
    random: Option<serde_json::Value>,
    pub regions: Vec<WriteRegion>,
    pub output_size: u64,
    append_offset: usize,
}
/// Rebuilds the archive at `iso[base..base + size]` with every crown model pack
/// expanded and the river turret scripts updated. Returns the recipe for the new archive and its length. It knows
/// nothing about the disc filesystem, so ISO9660 and GameCube images share it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_afs(
    iso: &[u8],
    base: usize,
    size: usize,
    four_player_spawns: bool,
    crown_drops: bool,
    random_crown_colours: bool,
    colour_seed: u32,
    graft: Option<&graft::Spec>,
    random: Option<RandomOptions>,
) -> Result<(Vec<Piece>, usize, Option<serde_json::Value>)> {
    let afs = iso
        .get(base..base + size)
        .ok_or_else(|| bad("NDMW.AFS outside image"))?;
    need(afs.get(..4) == Some(b"AFS\0"), "invalid archive signature")?;
    let count = u32at(afs, 4)? as usize;
    need(
        count > 0 && count < 65536 && 8 + 8 * count <= size,
        "invalid archive count",
    )?;
    let entries = (0..count)
        .map(|i| {
            Ok((
                u32at(afs, 8 + 8 * i)? as usize,
                u32at(afs, 12 + 8 * i)? as usize,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let first = entries[0].0;
    need(
        first >= 16 + 8 * count && first <= size,
        "invalid archive prefix",
    )?;
    let mut random = match random {
        Some(options) => {
            let layout = crate::release::detect(iso)?.layout_in(iso)?;
            Some(random::Randomiser::scan(
                afs,
                &entries,
                crate::table::EnemyTable::read(iso, &layout)?,
                options,
            )?)
        }
        None => None,
    };
    let mut prefix = afs[..first].to_vec();
    let mut pieces = Vec::new();
    let mut cursor = first;
    let mut newentries = Vec::new();
    let mut patched = Vec::new();
    let mut crown_packs = 0;
    let mut spawn_packs = 0;
    let mut drop_packs = 0;
    let mut river_packs = 0;
    let mut practice_packs = 0;
    let mut library = drops::Library::default();
    if crown_drops {
        for &(off, len) in &entries {
            let data = afs
                .get(off..off + len)
                .ok_or_else(|| bad("loot archive entry outside archive"))?;
            if memchr::memmem::find(data, b"beGeneraterData\0").is_some() {
                library.collect(data)?;
            }
        }
    }
    for (i, &(off, len)) in entries.iter().enumerate() {
        let next = entries.get(i + 1).map_or(size, |e| e.0);
        need(
            off <= next && len <= next - off && next <= size && off % 2048 == 0,
            "invalid archive entry extent/order",
        )?;
        newentries.push((cursor, len));
        let data = &afs[off..off + len];
        let edits = if memchr::memmem::find(data, b"i_rareicon").is_some() {
            crown_packs += 1;
            let edits = expand(data)?;
            if memchr::memmem::find(data, b"g_numepodz01\0").is_some() {
                river_packs += 1;
                let expanded = materialize(data, &edits)?;
                let edits = river::expand(&expanded, random_crown_colours, colour_seed)
                    .map_err(|e| bad(&format!("river archive entry {i}: {e}")))?;
                vec![Edit {
                    range: 0..data.len(),
                    bytes: materialize(&expanded, &edits)?,
                }]
            } else if four_player_spawns && practice::candidate(data) {
                let expanded = materialize(data, &edits)?;
                let more = practice::expand(&expanded)
                    .map_err(|e| bad(&format!("practice archive entry {i}: {e}")))?;
                if more.is_empty() {
                    edits
                } else {
                    practice_packs += 1;
                    vec![Edit {
                        range: 0..data.len(),
                        bytes: materialize(&expanded, &more)?,
                    }]
                }
            } else {
                edits
            }
        } else if (four_player_spawns || crown_drops)
            && memchr::memmem::find(data, b"beGeneraterData\0").is_some()
        {
            let mut bytes = data.to_vec();
            if four_player_spawns {
                let edits = spawns::expand(&bytes)
                    .map_err(|e| bad(&format!("spawn archive entry {i}: {e}")))?;
                if !edits.is_empty() {
                    spawn_packs += 1;
                    bytes = materialize(&bytes, &edits)?;
                }
            }
            if crown_drops {
                let edits = drops::expand(&bytes, &library)
                    .map_err(|e| bad(&format!("loot archive entry {i}: {e}")))?;
                if !edits.is_empty() {
                    drop_packs += 1;
                    bytes = materialize(&bytes, &edits)?;
                }
            }
            if bytes == data {
                Vec::new()
            } else {
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
        } else {
            Vec::new()
        };
        let edits = match graft {
            Some(g)
                if i == g.model_entry
                    || g.generator_entries.contains(&i)
                    || g.lua_edits.iter().any(|e| e.entry == i) =>
            {
                let current = if i == g.model_entry && !g.expand_model_pack {
                    data.to_vec()
                } else {
                    materialize(data, &edits)?
                };
                let mut bytes = current;
                if i == g.model_entry && !g.models.is_empty() {
                    let sources = std::iter::once((g.source_entry, &g.models))
                        .chain(g.extra_sources.iter().map(|(e, m)| (*e, m)));
                    for (src_entry, models) in sources {
                        // Models the stage already carries need no copy.
                        let mut missing = Vec::new();
                        for m in models {
                            if !graft::resident(&bytes, m)? {
                                missing.push(m.clone());
                            }
                        }
                        if missing.is_empty() {
                            continue;
                        }
                        let (soff, slen) = *entries
                            .get(src_entry)
                            .ok_or_else(|| bad("graft source entry outside archive"))?;
                        let source = afs
                            .get(soff..soff + slen)
                            .ok_or_else(|| bad("graft source outside archive"))?;
                        let (more, report) =
                            graft::graft(&bytes, source, &missing).map_err(|e| {
                                bad(&format!(
                                    "graft {missing:?} into entry {i} from {src_entry}: {e}"
                                ))
                            })?;
                        eprintln!("graft entry {i} <- {src_entry}: {report:?}");
                        bytes = materialize(&bytes, &more)?;
                    }
                } else if g.generator_entries.contains(&i) {
                    let (out, n) = graft::retarget(
                        &bytes,
                        &g.rows,
                        &g.spread,
                        &g.only_generators,
                        &g.generator_models,
                    )
                    .map_err(|e| bad(&format!("retarget entry {i}: {e}")))?;
                    need(n > 0, "no generator rows were retargeted")?;
                    eprintln!("retarget entry {i}: {n} generators");
                    bytes = out;
                }
                for e in g.lua_edits.iter().filter(|e| e.entry == i) {
                    let more = lua_names::rename_in_pack(&bytes, &e.scripts, &e.names)
                        .map_err(|err| bad(&format!("Lua rename in entry {i}: {err}")))?;
                    eprintln!("lua rename entry {i}: {:?}", e.names);
                    bytes = materialize(&bytes, &more)?;
                }
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
            _ => edits,
        };
        let edits = match random.as_mut() {
            Some(r) if r.is_stage(i) || r.plans_for(i) => {
                let current = match edits.as_slice() {
                    [e] if e.range == (0..data.len()) => e.bytes.clone(),
                    _ => materialize(data, &edits)?,
                };
                let bytes = if r.is_stage(i) {
                    r.stage(i, current, afs, &entries)
                        .map_err(|e| bad(&format!("randomise entry {i}: {e}")))?
                } else {
                    r.retarget(i, &current)
                        .map_err(|e| bad(&format!("randomise entry {i}: {e}")))?
                        .unwrap_or(current)
                };
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
            _ => edits,
        };
        if !edits.is_empty() {
            let mut q = 0;
            let mut newlen = len;
            for e in edits {
                need(e.range.start >= q, "overlapping archive edits")?;
                if e.range.start > q {
                    pieces.push(Piece::Source(base + off + q..base + off + e.range.start));
                }
                newlen = newlen + e.bytes.len() - e.range.len();
                pieces.push(Piece::Bytes(e.bytes));
                q = e.range.end;
            }
            if q < len {
                pieces.push(Piece::Source(base + off + q..base + off + len));
            }
            newentries[i].1 = newlen;
            cursor += newlen;
            let aligned = a2048(cursor);
            if aligned > cursor {
                pieces.push(Piece::Bytes(vec![0; aligned - cursor]));
            }
            cursor = aligned;
            let oldend = a2048(off + len);
            if next > oldend {
                pieces.push(Piece::Source(base + oldend..base + next));
                cursor += next - oldend;
            }
            patched.push(i);
        } else {
            pieces.push(Piece::Source(base + off..base + next));
            cursor += next - off;
        }
    }
    need(crown_packs == 85, "expected 85 crown model packs")?;
    need(river_packs == 1, "expected one Numenume River model pack")?;
    need(
        !four_player_spawns || spawn_packs > 0,
        "no four-player spawn profiles found",
    )?;
    need(
        !four_player_spawns || practice_packs == 1,
        "expected one practice graduation script",
    )?;
    need(
        !crown_drops || drop_packs > 0,
        "no custom loot profiles found",
    )?;
    for (i, &(off, len)) in newentries.iter().enumerate() {
        need(
            off <= u32::MAX as usize && len <= u32::MAX as usize,
            "archive exceeds format limit",
        )?;
        put(&mut prefix, 8 + 8 * i, off as u32);
        put(&mut prefix, 12 + 8 * i, len as u32);
    }
    for pos in [8 + 8 * count, first - 8] {
        let footer = u32at(afs, pos)? as usize;
        let len = u32at(afs, pos + 4)? as usize;
        if footer == 0 && len == 0 {
            continue;
        }
        need(
            footer > 0 && len > 0 && footer <= size && len <= size - footer,
            "invalid archive footer",
        )?;
        let owner = entries
            .iter()
            .rposition(|e| e.0 <= footer)
            .ok_or_else(|| bad("footer has no owner"))?;
        let (oldoff, oldlen) = entries[owner];
        let (newoff, newlen) = newentries[owner];
        let relocated = if patched.contains(&owner) {
            let end = a2048(oldoff + oldlen);
            need(footer >= end, "footer overlaps modified pack")?;
            a2048(newoff + newlen) + footer - end
        } else {
            newoff + footer - oldoff
        };
        put(&mut prefix, pos, relocated as u32);
    }
    pieces.insert(0, Piece::Bytes(prefix));
    if let Some(r) = &random {
        if std::env::var_os("DW4_RANDOM_LOG").is_some() {
            for line in &r.log {
                eprintln!("{line}");
            }
        }
    }
    Ok((
        pieces,
        cursor,
        random.as_ref().map(random::Randomiser::summary),
    ))
}

impl CrownPlan {
    pub fn build(iso: &[u8]) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, None, None)
    }
    /// Experimental: the usual crown expansion plus an enemy model graft.
    pub fn build_graft(iso: &[u8], spec: &graft::Spec) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, Some(spec), None)
    }
    /// Experimental: the usual crown expansion plus the enemy randomiser.
    pub fn build_random(iso: &[u8], options: RandomOptions) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, None, Some(options))
    }
    pub(crate) fn build_with_options(
        iso: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        graft: Option<&graft::Spec>,
        random: Option<RandomOptions>,
    ) -> Result<Self> {
        let file = iso9660::find_file(iso, "NDMW.AFS")?;
        let base = file.lba as usize * 2048;
        let size = file.size as usize;
        let (pieces, cursor, random) = rebuild_afs(
            iso,
            base,
            size,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            graft,
            random,
        )?;
        let sectors = u32at(iso, 16 * 2048 + 80)? as usize;
        let append_offset = sectors * 2048;
        need(append_offset == iso.len(), "unexpected trailing ISO data")?;
        let output_size = a2048(append_offset + cursor);
        need(
            output_size / 2048 <= u32::MAX as usize && cursor <= u32::MAX as usize,
            "expanded image exceeds format limit",
        )?;
        let mut extent = Vec::new();
        extent.extend((sectors as u32).to_le_bytes());
        extent.extend((sectors as u32).to_be_bytes());
        let mut length = Vec::new();
        length.extend((cursor as u32).to_le_bytes());
        length.extend((cursor as u32).to_be_bytes());
        let mut regions = vec![
            WriteRegion {
                offset: file.record_offset + 2,
                bytes: extent,
            },
            WriteRegion {
                offset: file.record_offset + 10,
                bytes: length,
            },
        ];
        for sector in 16..(iso.len() / 2048).min(64) {
            let p = sector * 2048;
            need(&iso[p + 1..p + 6] == b"CD001", "invalid volume descriptor")?;
            if iso[p] == 255 {
                break;
            }
            if iso[p] == 1 || iso[p] == 2 {
                let n = (output_size / 2048) as u32;
                let mut bytes = n.to_le_bytes().to_vec();
                bytes.extend(n.to_be_bytes());
                regions.push(WriteRegion {
                    offset: (p + 80) as u64,
                    bytes,
                });
            }
        }
        Ok(Self {
            pieces,
            regions,
            output_size: output_size as u64,
            append_offset,
            random,
        })
    }
    /// GameCube twin of [`CrownPlan::build`]: the rebuilt archive is appended
    /// after the last byte of the image and the FST entry repointed at it.
    pub fn build_gcm(disc: &[u8]) -> Result<Self> {
        Self::build_gcm_with_options(disc, false, false, false, 0, None)
    }
    pub(crate) fn build_gcm_with_options(
        disc: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        random: Option<RandomOptions>,
    ) -> Result<Self> {
        let file = gcm::find_file(disc, "ndmw.afs")?;
        let (base, size) = (file.offset as usize, file.size as usize);
        let (pieces, cursor, random) = rebuild_afs(
            disc,
            base,
            size,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            None,
            random,
        )?;
        let append_offset = disc.len();
        need(
            append_offset.is_multiple_of(4),
            "unaligned GameCube image length",
        )?;
        need(
            (append_offset + cursor) as u64 <= u32::MAX as u64,
            "expanded image exceeds format limit",
        )?;
        let regions = vec![
            WriteRegion {
                offset: file.offset_field,
                bytes: (append_offset as u32).to_be_bytes().to_vec(),
            },
            WriteRegion {
                offset: file.size_field,
                bytes: (cursor as u32).to_be_bytes().to_vec(),
            },
        ];
        Ok(Self {
            pieces,
            regions,
            output_size: (append_offset + cursor) as u64,
            append_offset,
            random,
        })
    }
    /// Append identification outside NDMW.AFS; preserve every resource offset and length.
    pub fn with_signature(
        self,
        release: crate::release::Release,
        plan: &crate::plan::PatchPlan,
    ) -> Result<Self> {
        let mut metadata = crate::signature::metadata(release, plan);
        if let Some(random) = &self.random {
            metadata["randomEnemies"] = random.clone();
        }
        self.finish_signature(release, metadata)
    }

    /// Store the original bytes of every overwritten region outside all game files.
    pub(crate) fn with_recovery(
        mut self,
        iso: &[u8],
        release: crate::release::Release,
        plan: &crate::plan::PatchPlan,
        regions: &[WriteRegion],
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Self> {
        let recovery = crate::update::capture(iso, regions, progress)?;
        let used: usize = self
            .pieces
            .iter()
            .map(|p| match p {
                Piece::Source(r) => r.len(),
                Piece::Bytes(b) => b.len(),
            })
            .sum();
        let offset = self.output_size as usize;
        let padding = offset
            .checked_sub(self.append_offset + used)
            .ok_or_else(|| bad("recovery overlaps archive"))?;
        self.pieces.push(Piece::Bytes(vec![0; padding]));
        let mut metadata = crate::signature::metadata(release, plan);
        if let Some(random) = &self.random {
            metadata["randomEnemies"] = random.clone();
        }
        metadata["recovery"] = serde_json::json!({
            "formatVersion": 1, "offset": offset, "length": recovery.len(),
            "sha1": crate::update::digest(&recovery), "originalSize": iso.len(),
            "regionCount": regions.len(), "archiveEnd": self.output_size,
        });
        self.pieces.push(Piece::Bytes(recovery));
        self.output_size = offset as u64
            + match self.pieces.last().unwrap() {
                Piece::Bytes(b) => b.len() as u64,
                _ => 0,
            };
        self.finish_signature(release, metadata)
    }

    fn finish_signature(
        mut self,
        release: crate::release::Release,
        metadata: serde_json::Value,
    ) -> Result<Self> {
        let offset = a2048(self.output_size as usize);
        let used: usize = self
            .pieces
            .iter()
            .map(|p| match p {
                Piece::Source(r) => r.len(),
                Piece::Bytes(b) => b.len(),
            })
            .sum();
        let padding = offset
            .checked_sub(self.append_offset + used)
            .ok_or_else(|| bad("signature overlaps archive"))?;
        self.pieces.push(Piece::Bytes(vec![0; padding]));
        self.pieces
            .push(Piece::Bytes(crate::signature::encode(&metadata)?));
        self.output_size = offset as u64 + crate::signature::SIZE as u64;
        if release.is_gamecube() {
            need(
                self.output_size <= u32::MAX as u64,
                "signed image exceeds GameCube format limit",
            )?;
        } else {
            let sectors = u32::try_from(self.output_size / 2048)
                .map_err(|_| bad("signed image exceeds ISO format limit"))?;
            for r in &mut self.regions {
                if r.offset >= 16 * 2048 && r.offset < 64 * 2048 && r.offset % 2048 == 80 {
                    r.bytes = [sectors.to_le_bytes(), sectors.to_be_bytes()].concat();
                }
            }
        }
        Ok(self)
    }
    /// Experimental: write the complete patched image (original bytes, appended archive, in-place regions).
    pub fn write_image(&self, iso: &[u8], out: &mut std::fs::File) -> Result<()> {
        use std::io::{Seek, SeekFrom};
        out.write_all(iso)?;
        self.append(iso, out, &mut |_, _| {})?;
        for r in &self.regions {
            out.seek(SeekFrom::Start(r.offset))?;
            out.write_all(&r.bytes)?;
        }
        Ok(())
    }
    pub fn append(
        &self,
        iso: &[u8],
        out: &mut impl Write,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<()> {
        let total = self.output_size - self.append_offset as u64;
        let mut done = 0;
        for piece in &self.pieces {
            let bytes = match piece {
                Piece::Source(r) => &iso[r.clone()],
                Piece::Bytes(b) => b,
            };
            for chunk in bytes.chunks(8 << 20) {
                out.write_all(chunk)?;
                done += chunk.len() as u64;
                progress(done, total);
            }
        }
        need(done <= total, "rebuilt archive exceeds image")?;
        out.write_all(&vec![0; (total - done) as usize])?;
        Ok(())
    }
    pub fn verify(&self, iso: &[u8], out: &[u8]) -> Result<()> {
        need(
            out.len() as u64 == self.output_size,
            "expanded image length mismatch",
        )?;
        let mut pos = self.append_offset;
        for piece in &self.pieces {
            let bytes = match piece {
                Piece::Source(r) => &iso[r.clone()],
                Piece::Bytes(b) => b,
            };
            need(
                out.get(pos..pos + bytes.len()) == Some(bytes),
                "rebuilt archive differs from plan",
            )?;
            pos += bytes.len();
        }
        need(out[pos..].iter().all(|b| *b == 0), "nonzero image padding")?;
        for r in &self.regions {
            need(
                out.get(r.offset as usize..r.offset as usize + r.bytes.len()) == Some(&r.bytes),
                "ISO extent metadata mismatch",
            )?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_packs_are_refused_without_panicking() {
        for n in [0, 4, 47, 48, 2048, 32768] {
            assert!(expand(&vec![0; n]).is_err());
        }
        let mut b = vec![0; 64];
        put(&mut b, 12, u32::MAX);
        assert!(expand(&b).is_err());
    }
}
