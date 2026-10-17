//! Expands the authored crown resources together: model and animation RAM,
//! private mutable scene roots, and a private two-word visibility mask.
//! No executable offsets or external game metadata are used. Original geometry,
//! original IDs and pool contents remain intact. Archive order is preserved.
use crate::{
    error::{Error, Result},
    gcm, iso9660,
    region::WriteRegion,
};
use std::{collections::BTreeMap, io::Write, ops::Range};

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
    let mut best = Vec::new();
    for p in memchr::memmem::find_iter(&b[..b.len().min(0x8000)], b"\0igObject\0") {
        let start = p + 1;
        let limit = b.len().min(start + 0x8000);
        let mut cur = start;
        let mut out = Vec::new();
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
            out.push(Class {
                name: name.into(),
                fields,
            });
            match next {
                Some(q) => cur = q,
                None => break,
            }
        }
        if out.len() > best.len() {
            best = out;
        }
    }
    need(
        best.len() == u32at(b, 12)? as usize,
        "type directory count mismatch",
    )?;
    Ok(best)
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
        }
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
        self.add(
            self.p.b[directory.range.clone()].to_vec(),
            r.patch(self.p.b, changes)?,
            false,
        )
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
        objects.extend(&self.p.b[self.p.pool..]);
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
            (6, self.pool.len()),
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
    pub regions: Vec<WriteRegion>,
    pub output_size: u64,
    append_offset: usize,
}
/// Rebuilds the archive at `iso[base..base + size]` with every crown model pack
/// expanded. Returns the recipe for the new archive and its length. It knows
/// nothing about the disc filesystem, so ISO9660 and GameCube images share it.
pub(crate) fn rebuild_afs(iso: &[u8], base: usize, size: usize) -> Result<(Vec<Piece>, usize)> {
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
    let mut prefix = afs[..first].to_vec();
    let mut pieces = Vec::new();
    let mut cursor = first;
    let mut newentries = Vec::new();
    let mut patched = Vec::new();
    for (i, &(off, len)) in entries.iter().enumerate() {
        let next = entries.get(i + 1).map_or(size, |e| e.0);
        need(
            off <= next && len <= next - off && next <= size && off % 2048 == 0,
            "invalid archive entry extent/order",
        )?;
        newentries.push((cursor, len));
        let data = &afs[off..off + len];
        if memchr::memmem::find(data, b"i_rareicon").is_some() {
            let edits = expand(data).map_err(|e| bad(&format!("archive entry {i}: {e}")))?;
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
    need(patched.len() == 85, "expected 85 crown model packs")?;
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
    Ok((pieces, cursor))
}

impl CrownPlan {
    pub fn build(iso: &[u8]) -> Result<Self> {
        let file = iso9660::find_file(iso, "NDMW.AFS")?;
        let base = file.lba as usize * 2048;
        let size = file.size as usize;
        let (pieces, cursor) = rebuild_afs(iso, base, size)?;
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
        })
    }
    /// GameCube twin of [`CrownPlan::build`]: the rebuilt archive is appended
    /// after the last byte of the image and the FST entry repointed at it.
    pub fn build_gcm(disc: &[u8]) -> Result<Self> {
        let file = gcm::find_file(disc, "ndmw.afs")?;
        let (base, size) = (file.offset as usize, file.size as usize);
        let (pieces, cursor) = rebuild_afs(disc, base, size)?;
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
        })
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
