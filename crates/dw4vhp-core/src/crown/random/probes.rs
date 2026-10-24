use super::*;

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_scripts -- --ignored --nocapture`
/// Every `xxx_<model>.lua` script of the stage packs, by prefix and model.
#[test]
#[ignore]
fn probe_scripts() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let n = u32at(afs, 4).unwrap() as usize;
    let mut seen: BTreeMap<(String, String), BTreeSet<usize>> = BTreeMap::new();
    for e in 0..n {
        let (o, l) = (
            u32at(afs, 8 + 8 * e).unwrap() as usize,
            u32at(afs, 12 + 8 * e).unwrap() as usize,
        );
        if !(1_500_000..=12_000_000).contains(&l)
            || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
        {
            continue;
        }
        let Ok(p) = Pack::parse(&afs[o..o + l]) else {
            continue;
        };
        for id in 0..p.ids.len() as u32 {
            let Some(i) = p.ids[id as usize] else {
                continue;
            };
            let r = &p.objects[i];
            if p.classes[r.class].name != "beLuaData" {
                continue;
            }
            let Some(name) = graft::name_of(&p, r) else {
                continue;
            };
            let name = String::from_utf8_lossy(name).into_owned();
            if let Some((prefix, rest)) = name.split_once('_') {
                if let Some(model) = rest.strip_suffix(".lua") {
                    if (model.starts_with("e_") || model.starts_with("f_")) && prefix.len() == 3 {
                        seen.entry((prefix.to_string(), model.to_string()))
                            .or_default()
                            .insert(e);
                    }
                }
            }
        }
    }
    let mut by_prefix: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((prefix, model), entries) in &seen {
        by_prefix
            .entry(prefix.clone())
            .or_default()
            .push(format!("{model}x{}", entries.len()));
    }
    for (prefix, models) in by_prefix {
        println!("{prefix}: {}", models.join(" "));
    }
}

/// `DW4_ENTRY=716 DW4_MATCH=026 ... cargo test --features devtools probe_bullets -- --ignored --nocapture`: everything
/// in one stage pack whose name has the text, and the string constants of the scripts of `DW4_MODEL`.
#[test]
#[ignore]
fn probe_bullets() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let e: usize = std::env::var("DW4_ENTRY").unwrap().parse().unwrap();
    let (o, l) = (
        u32at(afs, 8 + 8 * e).unwrap() as usize,
        u32at(afs, 12 + 8 * e).unwrap() as usize,
    );
    let p = Pack::parse(&afs[o..o + l]).unwrap();
    let needle = std::env::var("DW4_MATCH").unwrap();
    for id in 0..p.ids.len() as u32 {
        let Some(i) = p.ids[id as usize] else {
            continue;
        };
        let r = &p.objects[i];
        let Some(name) = graft::name_of(&p, r).map(|x| String::from_utf8_lossy(x).into_owned())
        else {
            continue;
        };
        if name.contains(&needle) {
            println!("OBJ {id} {} {name}", p.classes[r.class].name);
        }
    }
    for (class, name) in [
        ("beAction2Info", "x_026eblst01a"),
        ("igSwitch", "y_026emagc01a"),
    ] {
        if let Ok(Some(id)) = graft::find_named(&p, class, name) {
            let switch = if class == "igSwitch" {
                id
            } else {
                p.val(id, 7).unwrap()
            };
            let mut level = vec![switch];
            for depth in 0..5 {
                let mut next = Vec::new();
                for h in 0..p.ids.len() as u32 {
                    let Some(hi) = p.ids[h as usize] else {
                        continue;
                    };
                    let hr = &p.objects[hi];
                    for (n, memory) in graft::ref_slots(&p, hr) {
                        if memory {
                            continue;
                        }
                        let v = hr.val(p.b, n).unwrap();
                        let hit = level.contains(&v)
                            || (p.class(v, "igNodeList").is_ok()
                                && p.array(v).unwrap().iter().any(|c| level.contains(c)));
                        if hit && !next.contains(&h) {
                            println!(
                                "HOLDER {name} d{depth} <- {h} {} {:?} slot {n}",
                                p.classes[hr.class].name,
                                graft::name_of(&p, hr)
                                    .map(|x| String::from_utf8_lossy(x).into_owned())
                            );
                            next.push(h);
                        }
                    }
                }
                if next.len() > 12 {
                    break;
                }
                level = next;
            }
            match graft::parent_group(&p, switch) {
                Ok((group, _)) => println!(
                    "PARENT {name} -> {:?}",
                    graft::name_of(&p, p.rec(group).unwrap())
                        .map(|n| String::from_utf8_lossy(n).into_owned())
                ),
                Err(e) => println!("PARENT {name} -> none ({e:?})"),
            }
        }
    }
    for h in 0..p.ids.len() as u32 {
        let Some(hi) = p.ids[h as usize] else {
            continue;
        };
        let hr = &p.objects[hi];
        let class = p.classes[hr.class].name.as_str();
        let name = graft::name_of(&p, hr)
            .map(|x| String::from_utf8_lossy(x).into_owned())
            .unwrap_or_default();
        if (class == "igGroup" || class == "igAttrSet") && (name == "EF01" || name == "EF02") {
            for (n, memory) in graft::ref_slots(&p, hr) {
                if memory {
                    continue;
                }
                let v = hr.val(p.b, n).unwrap();
                if v != u32::MAX && p.class(v, "igNodeList").is_ok() {
                    let kids: Vec<String> = p
                        .array(v)
                        .unwrap()
                        .iter()
                        .map(|&c| {
                            let r = p.rec(c).unwrap();
                            format!(
                                "{c}:{}:{}",
                                p.classes[r.class].name,
                                graft::name_of(&p, r)
                                    .map(|x| String::from_utf8_lossy(x).into_owned())
                                    .unwrap_or_default()
                            )
                        })
                        .collect();
                    println!("GROUP {h} {class} {name} slot {n} kids {kids:?}");
                }
            }
        }
    }
    {
        fn tree(p: &Pack, id: u32, depth: usize) {
            let r = p.rec(id).unwrap();
            let name = graft::name_of(p, r)
                .map(|x| String::from_utf8_lossy(x).into_owned())
                .unwrap_or_default();
            println!(
                "TREE {}{id} {} {name}",
                "  ".repeat(depth),
                p.classes[r.class].name
            );
            if depth > 5 {
                return;
            }
            for (n, memory) in graft::ref_slots(p, r) {
                if memory {
                    continue;
                }
                let v = r.val(p.b, n).unwrap();
                if v != u32::MAX && p.class(v, "igNodeList").is_ok() {
                    for c in p.array(v).unwrap() {
                        tree(p, c, depth + 1)
                    }
                }
            }
        }
        if let Ok(t) = std::env::var("DW4_TREE") {
            for id in t.split(',') {
                tree(&p, id.parse().unwrap(), 0)
            }
        }
    }
    if let Ok(model) = std::env::var("DW4_MODEL") {
        for prefix in graft::MODEL_SCRIPTS {
            let name = format!("{prefix}{model}.lua");
            let Ok(Some(id)) = graft::find_named(&p, "beLuaData", &name) else {
                continue;
            };
            let object = p.val(id, 3).unwrap();
            let (size, mem) = (
                p.val(object, 2).unwrap() as usize,
                p.val(object, 3).unwrap(),
            );
            let range = p.memory[mem as usize].as_ref().unwrap();
            let raw = &p.b[range.start..range.start + size];
            let consts = super::lua_names::constants(raw).unwrap();
            let strings: Vec<String> = consts
                .iter()
                .map(|c| String::from_utf8_lossy(&c.text).into_owned())
                .collect();
            println!("LUA {name} {strings:?}");
        }
    }
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_audit -- --ignored --nocapture`: for every enemy model in
/// every stage, the named objects its copied records and scripts mention by name but the copy leaves behind.
#[test]
#[ignore]
fn probe_audit() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let n = u32at(afs, 4).unwrap() as usize;
    // model -> (class, name) -> stages missing it
    let mut missing: BTreeMap<String, BTreeMap<(String, String), BTreeSet<usize>>> =
        BTreeMap::new();
    for e in 0..n {
        let (o, l) = (
            u32at(afs, 8 + 8 * e).unwrap() as usize,
            u32at(afs, 12 + 8 * e).unwrap() as usize,
        );
        if !(1_500_000..=12_000_000).contains(&l)
            || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
        {
            continue;
        }
        let Ok(sp) = Pack::parse(&afs[o..o + l]) else {
            continue;
        };
        let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
        let mut named: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for id in 0..sp.ids.len() as u32 {
            let Some(i) = sp.ids[id as usize] else {
                continue;
            };
            if let Some(name) =
                graft::name_of(&sp, &sp.objects[i]).map(|x| String::from_utf8_lossy(x).into_owned())
            {
                if name.len() >= 4 {
                    named.entry(name).or_default().push(id);
                }
            }
        }
        let models: Vec<String> = sp
            .array(root)
            .unwrap()
            .into_iter()
            .filter_map(|id| {
                let r = sp.rec(id).ok()?;
                (sp.classes[r.class].name == "beModelCtrlInfo")
                    .then(|| {
                        graft::name_of(&sp, r).map(|x| String::from_utf8_lossy(x).into_owned())
                    })
                    .flatten()
            })
            .filter(|m| m.starts_with("e_"))
            .collect();
        for m in models {
            let Ok((mut roots, _)) = graft::model_roots(&sp, &m) else {
                continue;
            };
            if std::env::var("DW4_AUDIT_BARE").is_ok() {
                roots.clear();
                for class in [
                    "beModelCtrlInfo",
                    "beAction2Info",
                    "igAnimationDatabase",
                    "beShadow01Info",
                    "beHitLandDelivInfo",
                ] {
                    roots.extend(graft::find_named(&sp, class, &m).ok().flatten());
                }
                for prefix in graft::MODEL_SCRIPTS {
                    roots.extend(
                        graft::find_named(&sp, "beLuaData", &format!("{prefix}{m}.lua"))
                            .ok()
                            .flatten(),
                    );
                }
            }
            let Ok((ids, _)) = graft::closure(&sp, &roots) else {
                continue;
            };
            let mut words: BTreeSet<String> = BTreeSet::new();
            for &id in &ids {
                if let Some(range) = sp.memory.get(id as usize).and_then(|r| r.clone()) {
                    let b = &sp.b[range];
                    let mut run = Vec::new();
                    for &c in b.iter().chain(std::iter::once(&0u8)) {
                        if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
                            run.push(c);
                        } else {
                            if run.len() >= 4 {
                                words.insert(String::from_utf8_lossy(&run).into_owned());
                            }
                            run.clear();
                        }
                    }
                }
            }
            if let Some((_, number)) = graft::EFFECT_NUMBERS.iter().find(|(name, _)| *name == m) {
                let mut attached = BTreeSet::new();
                if let Ok(units) = graft::scene_units(&sp, &m) {
                    attached.extend(units);
                }
                for (name, hits) in &named {
                    let own = name.contains(&format!("_{number}e"))
                        || name.contains(&format!("{number}e{}", &m[2..]));
                    if own {
                        for &h in hits {
                            if !ids.contains(&h)
                                && !name.ends_with("Shape1")
                                && !name.contains("_0Shape")
                            {
                                let class = sp.classes[sp.rec(h).unwrap().class].name.clone();
                                missing
                                    .entry(m.clone())
                                    .or_default()
                                    .entry((class, format!("NUM {name}")))
                                    .or_default()
                                    .insert(e);
                            }
                        }
                    }
                }
                let _ = attached;
            }
            for w in words {
                let key = w.strip_suffix(".lua").unwrap_or(&w).to_string();
                for cand in [&w, &key] {
                    if let Some(hits) = named.get(cand) {
                        let outside: Vec<u32> =
                            hits.iter().copied().filter(|h| !ids.contains(h)).collect();
                        if !outside.is_empty() && hits.len() <= 8 {
                            let class = sp.classes[sp.rec(outside[0]).unwrap().class].name.clone();
                            missing
                                .entry(m.clone())
                                .or_default()
                                .entry((class, cand.clone()))
                                .or_default()
                                .insert(e);
                        }
                    }
                }
            }
        }
    }
    for (m, items) in missing {
        for ((class, name), stages) in items {
            println!("AUDIT {m} {class} {name} stages={}", stages.len());
        }
    }
}

/// `DW4_PROBE_ISO=<iso|gcm|ciso> cargo test --features devtools probe_ids -- --ignored --nocapture`: object-ID budget of stage 417
/// and the donors that went into it.
#[test]
#[ignore]
fn probe_ids() {
    use crate::container::{detect, unpack, Format};
    let path = std::path::PathBuf::from(std::env::var("DW4_PROBE_ISO").unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let raw = match detect(&path).unwrap() {
        Format::Raw => path.clone(),
        format => {
            let out = tmp.path().join("disc.iso");
            unpack(format, &path, &out, &mut |_, _| {}).unwrap();
            out
        }
    };
    let iso = std::fs::read(&raw).unwrap();
    let (base, size) = match crate::iso9660::find_file(&iso, "NDMW.AFS") {
        Ok(f) => (f.lba as usize * 2048, f.size as usize),
        Err(_) => {
            let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
            (f.offset as usize, f.size as usize)
        }
    };
    let afs = &iso[base..base + size];
    let entry = |e: usize| {
        (
            u32at(afs, 8 + 8 * e).unwrap() as usize,
            u32at(afs, 12 + 8 * e).unwrap() as usize,
        )
    };
    let ids = |e: usize| {
        let (o, l) = entry(e);
        Pack::parse(&afs[o..o + l])
            .map(|p| (p.ids.len(), l))
            .unwrap()
    };
    println!(
        "IDS destination 417: ids {} bytes {}",
        ids(417).0,
        ids(417).1
    );
    {
        let (o, l) = entry(417);
        let sp = Pack::parse(&afs[o..o + l]).unwrap();
        let mut blocks = 0;
        for id in 0..sp.ids.len() {
            match sp.ids[id] {
                Some(i) => {
                    let r = &sp.objects[i];
                    let class = sp.classes[r.class].name.as_str();
                    println!("CLS {class}");
                    if let Some(n) = graft::name_of(&sp, r) {
                        println!("NAME {class} {}", String::from_utf8_lossy(n));
                    }
                }
                None => blocks += 1,
            }
        }
        println!("CLS <memory blocks> x{blocks}");
    }
    let mut total = ids(417).0;
    for (e, m) in [(614usize, "e_goburi"), (210, "e_mecha1"), (187, "e_scum")] {
        let (o, l) = entry(e);
        let sp = Pack::parse(&afs[o..o + l]).unwrap();
        let (mut roots, _) = graft::model_roots(&sp, m).unwrap();
        roots.extend(graft::scene_units(&sp, m).unwrap());
        let (closure, _) = graft::closure(&sp, &roots).unwrap();
        total += closure.len();
        println!(
            "IDS donor {m} from {e}: pack ids {} closure {} running total {total} (limit 65535)",
            sp.ids.len(),
            closure.len()
        );
    }
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_numbers -- --ignored --nocapture`: per stage, residents and effect numbers.
#[test]
#[ignore]
fn probe_numbers() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let n = u32at(afs, 4).unwrap() as usize;
    for e in 0..n {
        let (o, l) = (
            u32at(afs, 8 + 8 * e).unwrap() as usize,
            u32at(afs, 12 + 8 * e).unwrap() as usize,
        );
        if !(1_500_000..=12_000_000).contains(&l)
            || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
        {
            continue;
        }
        let Ok(sp) = Pack::parse(&afs[o..o + l]) else {
            continue;
        };
        let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
        let (mut res, mut nums) = (BTreeSet::new(), BTreeSet::new());
        for id in sp.array(root).unwrap() {
            let r = sp.rec(id).unwrap();
            let class = sp.classes[r.class].name.as_str();
            let Some(name) =
                graft::name_of(&sp, r).map(|x| String::from_utf8_lossy(x).into_owned())
            else {
                continue;
            };
            if class == "beModelCtrlInfo" && (name.starts_with("e_") || name.starts_with("f_")) {
                res.insert(name.clone());
            }
            if (name.starts_with("x_") || name.starts_with("y_"))
                && name.len() > 5
                && name.as_bytes()[5] == b'e'
                && name[2..5].bytes().all(|c| c.is_ascii_digit())
            {
                nums.insert(name[2..5].to_string());
            }
        }
        println!("NUM {e} {res:?} {nums:?}");
    }
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_effects -- --ignored --nocapture`: effect infos stage A has that stage B lacks.
#[test]
#[ignore]
fn probe_effects() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let names = |entry: usize| {
        let (o, l) = (
            u32at(afs, 8 + 8 * entry).unwrap() as usize,
            u32at(afs, 12 + 8 * entry).unwrap() as usize,
        );
        let sp = Pack::parse(&afs[o..o + l]).unwrap();
        let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
        let mut out = BTreeSet::new();
        for e in sp.array(root).unwrap() {
            let r = sp.rec(e).unwrap();
            let class = sp.classes[r.class].name.clone();
            let named = graft::name_of(&sp, r)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            if std::env::var("DW4_ALL").is_ok() && named.contains("026") {
                out.insert(format!("ANY {class}:{named}"));
            }
            if ["beParticleCtrl2Info", "ParticleInfo", "beActionStarterInfo"]
                .contains(&class.as_str())
            {
                out.insert(format!(
                    "{class}:{}",
                    graft::name_of(&sp, r)
                        .map(|n| String::from_utf8_lossy(n).into_owned())
                        .unwrap_or_default()
                ));
            }
        }
        out
    };
    let a: usize = std::env::var("DW4_A").unwrap().parse().unwrap();
    let b: usize = std::env::var("DW4_B").unwrap().parse().unwrap();
    let (sa, sb) = (names(a), names(b));
    println!("only in {a}: {:?}", sa.difference(&sb).collect::<Vec<_>>());
    println!("only in {b}: {:?}", sb.difference(&sa).collect::<Vec<_>>());
}

/// `DW4_RETAIL_USA_ISO=... DW4_PROBE_MODEL=e_infermo DW4_PROBE_ENTRY=387 cargo test --features devtools probe_names -- --ignored --nocapture`
/// Which infos of the source stage does the model refer to by name, without owning them?
#[test]
#[ignore]
fn probe_names() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let model = std::env::var("DW4_PROBE_MODEL").unwrap();
    let entry: usize = std::env::var("DW4_PROBE_ENTRY").unwrap().parse().unwrap();
    let (o, l) = (
        u32at(afs, 8 + 8 * entry).unwrap() as usize,
        u32at(afs, 12 + 8 * entry).unwrap() as usize,
    );
    let sp = Pack::parse(&afs[o..o + l]).unwrap();
    let mut roots = vec![];
    for class in [
        "beModelCtrlInfo",
        "beAction2Info",
        "igAnimationDatabase",
        "beShadow01Info",
        "beHitLandDelivInfo",
    ] {
        roots.push(graft::find_named(&sp, class, &model).unwrap().unwrap());
    }
    for prefix in ["hit_", "mag_"] {
        if let Some(id) =
            graft::find_named(&sp, "beLuaData", &format!("{prefix}{model}.lua")).unwrap()
        {
            roots.push(id);
        }
    }
    let (ids, _) = graft::closure(&sp, &roots).unwrap();
    // every string the closure carries: string fields and Lua source constants
    let mut words: BTreeSet<String> = BTreeSet::new();
    for &id in &ids {
        if let Some(i) = sp.ids[id as usize] {
            let r = &sp.objects[i];
            for (n, range) in &r.fields {
                let c = &sp.classes[r.class];
                if c.fields.iter().any(|&(t, nn, _)| nn == *n && t == 4) && range.len() > 4 {
                    let t = String::from_utf8_lossy(&sp.b[range.start + 4..range.end]);
                    words.insert(t.trim_end_matches('\0').to_string());
                }
            }
        } else if let Some(r) = sp.memory[id as usize].clone() {
            for w in sp.b[r].split(|c| !(c.is_ascii_alphanumeric() || *c == b'_')) {
                if w.len() >= 4 {
                    words.insert(String::from_utf8_lossy(w).into_owned());
                }
            }
        }
    }
    println!("{} ids, {} distinct strings", ids.len(), words.len());
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for &id in &ids {
        if let Some(i) = sp.ids[id as usize] {
            *by.entry(sp.classes[sp.objects[i].class].name.clone())
                .or_default() += 1;
        }
    }
    println!("classes: {by:?}");
    let mut scripts = vec![];
    for id in 0..sp.ids.len() as u32 {
        if let Some(i) = sp.ids[id as usize] {
            let r = &sp.objects[i];
            if sp.classes[r.class].name == "beLuaData" {
                if let Some(n) = graft::name_of(&sp, r) {
                    let n = String::from_utf8_lossy(n).into_owned();
                    if n.contains(model.trim_start_matches("e_")) {
                        scripts.push(n);
                    }
                }
            }
        }
    }
    println!("scripts naming the model: {scripts:?}");
    for name in &scripts {
        if name.starts_with("hit_") {
            continue;
        }
        let id = graft::find_named(&sp, "beLuaData", name).unwrap().unwrap();
        let object = sp.val(id, 3).unwrap();
        let (size, mem) = (
            sp.val(object, 2).unwrap() as usize,
            sp.val(object, 3).unwrap(),
        );
        let range = sp.memory[mem as usize].clone().unwrap();
        let code = &sp.b[range.start..range.start + size];
        let words: Vec<String> = code
            .split(|c| !(c.is_ascii_graphic() || *c == b' '))
            .filter(|w| w.len() >= 3)
            .map(|w| String::from_utf8_lossy(w).into_owned())
            .collect();
        println!("{name} ({size} B): {words:?}");
    }
    let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
    for e in sp.array(root).unwrap() {
        if ids.contains(&e) {
            continue;
        }
        let r = sp.rec(e).unwrap();
        let name = graft::name_of(&sp, r)
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        if !name.is_empty() && words.contains(&name) {
            println!("  refers to {} {name}", sp.classes[r.class].name);
        }
    }
    let mut effects: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in sp.array(root).unwrap() {
        let r = sp.rec(e).unwrap();
        let class = sp.classes[r.class].name.clone();
        if [
            "beParticleCtrl2Info",
            "ParticleInfo",
            "beActionStarterInfo",
            "beHitLandModelInfo",
            "beSwitchCtrlInfo",
            "beTextureCtrlInfo",
        ]
        .contains(&class.as_str())
        {
            let name = graft::name_of(&sp, r)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            effects.entry(class).or_default().push(name);
        }
    }
    println!("effect infos in the stage: {effects:?}");
    println!(
        "strings: {:?}",
        words
            .iter()
            .filter(|w| w.len() > 3)
            .take(400)
            .collect::<Vec<_>>()
    );
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_classes -- --ignored --nocapture`
#[test]
#[ignore]
fn probe_classes() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let ent = |i: usize| {
        (
            u32at(afs, 8 + 8 * i).unwrap() as usize,
            u32at(afs, 12 + 8 * i).unwrap() as usize,
        )
    };
    for e in [179, 233, 288, 621, 665, 723] {
        let (o, l) = ent(e);
        let b = &afs[o..o + l];
        let (classes, _, end) = class_table(b).unwrap();
        let names = u32at(b, end).unwrap() as usize;
        println!(
            "entry {e}: words {:?} classes {} table end {end} names {names} -> dirs at {} ; w2 vs end: {}",
            (0..8).map(|i| u32at(b, 4 * i).unwrap()).collect::<Vec<_>>(),
            classes.len(),
            end + 8 + names,
            (end + 8 + names) as i64 - u32at(b, 8).unwrap() as i64
        );
    }
    {
        let (o, l) = ent(621);
        let b = &afs[o..o + l];
        let w2 = u32at(b, 8).unwrap() as usize;
        let pk = Pack::parse(b).unwrap();
        // classes whose table entry starts at or after word 2
        let mut at = b.windows(10).position(|w| w == b"\0igObject\0").unwrap() + 1;
        let mut tail = vec![];
        for (i, c) in pk.classes.iter().enumerate() {
            if at >= w2 {
                tail.push((i, c.name.clone()));
            }
            let pad = (at + c.name.len() + 2) & !1;
            at = pad + 6 * c.fields.len();
        }
        println!(
            "w2 {w2}: classes at/after it: {tail:?}; total {}",
            pk.classes.len()
        );
        let used: std::collections::BTreeSet<usize> = pk.objects.iter().map(|r| r.class).collect();
        println!(
            "object classes in tail: {:?}",
            tail.iter()
                .filter(|(i, _)| used.contains(i))
                .collect::<Vec<_>>()
        );
        let dirc: std::collections::BTreeSet<usize> = pk.dirs.iter().map(|r| r.class).collect();
        println!("dir classes {dirc:?}");
    }
    let (o, l) = ent(665);
    let dst = &afs[o..o + l];
    let (so, sl) = ent(210);
    let src = Pack::parse(&afs[so..so + sl]).unwrap();
    let (classes, _, end) = class_table(dst).unwrap();
    println!(
        "665: {} classes, table ends at {end}, next bytes {:?}",
        classes.len(),
        &dst[end..end + 16]
    );
    let want = ["beModelCtrlAIMAP"];
    let defs = class_defs(src.b).unwrap();
    let add: Vec<ClassDef> = want
        .iter()
        .filter(|w| !classes.iter().any(|c| &c.name == *w))
        .map(|w| defs.iter().find(|c| &c.class.name == w).unwrap().clone())
        .collect();
    let out = add_classes(dst, &add).unwrap();
    let p = Pack::parse(&out).unwrap();
    println!(
        "after: {} classes; ids {} vs {}",
        p.classes.len(),
        p.ids.len(),
        Pack::parse(dst).unwrap().ids.len()
    );
    assert!(p.classes.iter().any(|c| c.name == "beModelCtrlAIMAP"));
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_census -- --ignored --nocapture`
#[test]
#[ignore]
fn probe_census() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let n = u32at(afs, 4).unwrap() as usize;
    let entries: Vec<(usize, usize)> = (0..n)
        .map(|i| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        })
        .collect();
    let layout = crate::release::detect(&iso)
        .unwrap()
        .layout_in(&iso)
        .unwrap();
    let table = EnemyTable::read(&iso, &layout).unwrap();
    let r = Randomiser::scan(
        afs,
        &entries,
        table,
        Options {
            seed: 1,
            donors: 3,
            warp: None,
            only: None,
        },
    )
    .unwrap();
    println!(
        "donors: {:?}",
        r.donors
            .iter()
            .map(|(m, d)| (*m, d.source, d.ram, d.profile.scene.clone()))
            .collect::<Vec<_>>()
    );
    for (&e, st) in &r.stages {
        let (o, l) = entries[e];
        let dp = Pack::parse(&afs[o..o + l]).unwrap();
        let em01 = graft::has_group(&dp, b"EM01");
        let mut ok = vec![];
        let mut no = vec![];
        for ((m, _), d) in &r.donors {
            if st.residents.contains_key(*m) {
                continue;
            }
            match d.profile.misfit(&dp) {
                None => ok.push(*m),
                Some(w) => no.push(format!("{m}:{}", w.chars().take(28).collect::<String>())),
            }
        }
        println!(
            "stage {e}: EM01={em01} residents={:?} fit={} [{}] nofit={}",
            st.residents.keys().collect::<Vec<_>>(),
            ok.len(),
            ok.join(","),
            if ok.is_empty() {
                no.join(" ")
            } else {
                no.len().to_string()
            }
        );
    }
}

/// `DW4_RETAIL_USA_ISO=... cargo test --features devtools probe_fields -- --ignored --nocapture`
#[test]
#[ignore]
fn probe_fields() {
    let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let n = u32at(afs, 4).unwrap() as usize;
    let ent = |i: usize| {
        (
            u32at(afs, 8 + 8 * i).unwrap() as usize,
            u32at(afs, 12 + 8 * i).unwrap() as usize,
        )
    };
    let (o, l) = ent(622);
    let p = Pack::parse(&afs[o..o + l]).unwrap();
    let _ = n;
    for cls in [
        "beGeneraterData",
        "beGeneraterPlayerData",
        "beGeneraterPlayerDataList",
    ] {
        let c = p.classes.iter().find(|c| c.name == cls).unwrap();
        println!("{cls}: {:?}", c.fields);
    }
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let r = &p.objects[*index];
        if p.classes[r.class].name == "beGeneraterData" {
            println!("gen id {id}: {:?}", r.fields);
            let seq = r.field(8).unwrap();
            println!("{:?}", &p.b[seq.clone()]);
            break;
        }
    }
}
