//! IGB class metadata, object/memory directories, references and lossless edits.
//! Game-specific feature policies belong in `crown`, not in this module.
use crate::error::{Error, Result};
use std::{collections::BTreeMap, ops::Range};

pub(crate) const NULL: u32 = u32::MAX;
pub(crate) fn bad(s: &str) -> Error {
    Error::CrownData(s.into())
}
pub(crate) fn need(ok: bool, s: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(bad(s))
    }
}
pub(crate) fn u32at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4)
            .ok_or_else(|| bad("truncated word"))?
            .try_into()
            .unwrap(),
    ))
}
pub(crate) fn u16at(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2)
            .ok_or_else(|| bad("truncated descriptor"))?
            .try_into()
            .unwrap(),
    ))
}
pub(crate) fn put(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
pub(crate) fn a4(n: usize) -> usize {
    (n + 3) & !3
}
pub(crate) fn a2048(n: usize) -> usize {
    (n + 2047) & !2047
}
#[derive(Clone)]
pub(crate) struct Class {
    pub(crate) name: String,
    pub(crate) fields: Vec<(u16, u16, u16)>,
}
#[derive(Clone)]
pub(crate) struct Record {
    pub(crate) class: usize,
    pub(crate) range: Range<usize>,
    pub(crate) fields: BTreeMap<u16, Range<usize>>,
}
impl Record {
    pub(crate) fn field(&self, n: u16) -> Result<Range<usize>> {
        self.fields
            .get(&n)
            .cloned()
            .ok_or_else(|| bad("missing field"))
    }
    pub(crate) fn val(&self, b: &[u8], n: u16) -> Result<u32> {
        let r = self.field(n)?;
        need(r.len() == 4, "unexpected field width")?;
        u32at(b, r.start)
    }
    pub(crate) fn named(&self, b: &[u8], name: &str) -> bool {
        self.field(2)
            .ok()
            .and_then(|r| b.get(r.start + 4..r.end))
            .is_some_and(|s| s.split(|c| *c == 0).next() == Some(name.as_bytes()))
    }
    pub(crate) fn patch(&self, b: &[u8], changes: &[(u16, u32)]) -> Result<Vec<u8>> {
        let mut out = b[self.range.clone()].to_vec();
        for &(n, v) in changes {
            let r = self.field(n)?;
            need(r.len() == 4, "unexpected patch width")?;
            put(&mut out, r.start - self.range.start, v);
        }
        Ok(out)
    }
}
pub(crate) fn ident(b: &[u8], p: usize) -> Option<&str> {
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
pub(crate) fn classes(b: &[u8]) -> Result<Vec<Class>> {
    class_table(b).map(|(classes, _, _)| classes)
}
/// The class table, and the offsets where it starts and ends.
pub(crate) fn class_table(b: &[u8]) -> Result<(Vec<Class>, usize, usize)> {
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
pub(crate) struct ClassDef {
    pub(crate) class: Class,
    pub(crate) parent: Option<String>,
    pub(crate) slots: u32,
}
/// The hierarchy record of every class of the pack.
pub(crate) fn class_defs(b: &[u8]) -> Result<Vec<ClassDef>> {
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
pub(crate) fn add_classes(b: &[u8], add: &[ClassDef]) -> Result<Vec<u8>> {
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
pub(crate) fn record(b: &[u8], p: usize, classes: &[Class]) -> Result<Record> {
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
pub(crate) struct Pack<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) classes: Vec<Class>,
    pub(crate) dirs: Vec<Record>,
    pub(crate) objects: Vec<Record>,
    pub(crate) ids: Vec<Option<usize>>,
    pub(crate) memory: Vec<Option<Range<usize>>>,
    pub(crate) mapping: Vec<u16>,
    pub(crate) table: usize,
    pub(crate) pool: usize,
}
impl<'a> Pack<'a> {
    pub(crate) fn parse(b: &'a [u8]) -> Result<Self> {
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
    pub(crate) fn rec(&self, id: u32) -> Result<&Record> {
        let idx = self
            .ids
            .get(id as usize)
            .and_then(|x| *x)
            .ok_or_else(|| bad("expected object ID"))?;
        Ok(&self.objects[idx])
    }
    pub(crate) fn class(&self, id: u32, name: &str) -> Result<()> {
        need(
            self.classes[self.rec(id)?.class].name == name,
            "unexpected crown resource class",
        )
    }
    pub(crate) fn val(&self, id: u32, n: u16) -> Result<u32> {
        self.rec(id)?.val(self.b, n)
    }
    pub(crate) fn array(&self, id: u32) -> Result<Vec<u32>> {
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
    pub(crate) fn named(&self, class: &str, name: &str) -> Result<u32> {
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
pub(crate) struct Edit {
    pub(crate) range: Range<usize>,
    pub(crate) bytes: Vec<u8>,
}
pub(crate) struct Builder<'p, 'a> {
    pub(crate) p: &'p Pack<'a>,
    pub(crate) dirs: Vec<u8>,
    pub(crate) objects: Vec<u8>,
    pub(crate) pool: Vec<u8>,
    pub(crate) mapping: Vec<u16>,
    pub(crate) object_count: u32,
    pub(crate) directory_count: usize,
    pub(crate) reusable_dirs: Option<BTreeMap<Vec<u8>, u16>>,
    pub(crate) remapped: BTreeMap<usize, u16>,
    pub(crate) edits: Vec<Edit>,
    /// Bytes inserted at the end of existing memory blocks: (offset in the pool, bytes).
    pub(crate) grows: Vec<(usize, Vec<u8>)>,
}
impl<'p, 'a> Builder<'p, 'a> {
    pub(crate) fn new(p: &'p Pack<'a>) -> Self {
        Self {
            p,
            dirs: vec![],
            objects: vec![],
            pool: vec![],
            mapping: vec![],
            object_count: 0,
            directory_count: 0,
            reusable_dirs: None,
            remapped: BTreeMap::new(),
            edits: vec![],
            grows: vec![],
        }
    }
    /// Instance copies can share identical directory descriptors. The remap
    /// indexes are 16-bit; object IDs and references remain 32-bit.
    pub(crate) fn reuse_directories(mut self) -> Self {
        self.reusable_dirs = Some(
            self.p
                .dirs
                .iter()
                .enumerate()
                .map(|(i, d)| (self.p.b[d.range.clone()].to_vec(), i as u16))
                .collect(),
        );
        self
    }
    /// Where blocks were grown, as (offset in the pool, length), in pool order.
    pub(crate) fn grown(&self) -> Vec<(usize, usize)> {
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
    pub(crate) fn grow_mem(&mut self, block: u32, extra: Vec<u8>) -> Result<()> {
        need(extra.len().is_multiple_of(4), "unaligned block growth")?;
        if extra.is_empty() {
            return Ok(());
        }
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
        let bytes = dir.patch(self.p.b, &[(7, (size + extra.len()) as u32)])?;
        if self
            .p
            .mapping
            .iter()
            .filter(|&&d| d as usize == idx)
            .count()
            > 1
        {
            // Equal-sized blocks can share one directory descriptor. Give only the grown
            // block a private descriptor; its ID and the owning object's reference stay fixed.
            let index = self.p.dirs.len() + self.directory_count;
            need(
                index <= u16::MAX as usize,
                "expanded directory exceeds 16-bit index",
            )?;
            self.remapped.insert(block as usize, index as u16);
            self.directory_count += 1;
            self.dirs.extend(bytes);
        } else {
            self.edits.push(Edit {
                range: dir.range.clone(),
                bytes,
            });
        }
        self.grows.push((range.end - self.p.pool, extra));
        Ok(())
    }
    pub(crate) fn add(&mut self, dir: Vec<u8>, payload: Vec<u8>, memory: bool) -> Result<u32> {
        let id = self.p.ids.len() + self.mapping.len();
        let reused = self
            .reusable_dirs
            .as_ref()
            .and_then(|dirs| dirs.get(&dir))
            .copied();
        let idx = match reused {
            Some(idx) => idx,
            None => {
                let idx = self.p.dirs.len() + self.directory_count;
                need(
                    idx < u16::MAX as usize,
                    "expanded directory exceeds 16-bit index",
                )?;
                self.directory_count += 1;
                if let Some(dirs) = &mut self.reusable_dirs {
                    dirs.insert(dir.clone(), idx as u16);
                }
                self.dirs.extend(dir);
                idx as u16
            }
        };
        self.mapping.push(idx);
        if memory {
            self.pool.extend(&payload);
            self.pool.resize(a4(self.pool.len()), 0);
        } else {
            self.objects.extend(payload);
            self.object_count += 1;
        }
        Ok(id as u32)
    }
    pub(crate) fn obj(&mut self, id: u32, changes: &[(u16, u32)]) -> Result<u32> {
        let raw = self.p.rec(id)?.patch(self.p.b, changes)?;
        self.obj_data(id, raw)
    }
    pub(crate) fn obj_data(&mut self, id: u32, raw: Vec<u8>) -> Result<u32> {
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
    pub(crate) fn mem(&mut self, bytes: Vec<u8>, template: u32) -> Result<u32> {
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
    pub(crate) fn modify(&mut self, id: u32, changes: &[(u16, u32)]) -> Result<()> {
        let r = self.p.rec(id)?;
        self.edits.push(Edit {
            range: r.range.clone(),
            bytes: r.patch(self.p.b, changes)?,
        });
        Ok(())
    }
    pub(crate) fn finish(mut self, template: u32) -> Result<Vec<Edit>> {
        if !self.mapping.len().is_multiple_of(2) {
            // The template can describe object references. Even unowned blocks
            // are resolved by the loader, so zero would retain object ID 0 (or
            // treat memory ID 0 as an object and corrupt its second word).
            self.mem(words(&[NULL]), template)?;
        }
        let n = (self.p.ids.len() + self.mapping.len()) as u32;
        let mut table = self.dirs.clone();
        table.extend((8 + 2 * n).to_le_bytes());
        table.extend(n.to_le_bytes());
        for (id, &index) in self.p.mapping.iter().enumerate() {
            table.extend(
                self.remapped
                    .get(&id)
                    .copied()
                    .unwrap_or(index)
                    .to_le_bytes(),
            );
        }
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
            (1, self.directory_count),
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
pub(crate) fn words(ids: &[u32]) -> Vec<u8> {
    ids.iter().flat_map(|x| x.to_le_bytes()).collect()
}
pub(crate) fn materialize(b: &[u8], edits: &[Edit]) -> Result<Vec<u8>> {
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

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
