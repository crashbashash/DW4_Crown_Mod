//! Copy enemy model subgraphs and retarget generator rows for the randomizer.
//! The public graft specifications also support focused development fixtures.
use super::*;
use crate::formats::igb::Class;
use std::collections::{BTreeSet, HashMap};

/// The records the engine registers per model, in the pack's root `igInfoList`.
const REGISTERED: [&str; 5] = [
    "beModelCtrlInfo",
    "beAction2Info",
    "igAnimationDatabase",
    "beShadow01Info",
    "beHitLandDelivInfo",
];
/// The scripts a model carries, named `<prefix><model>.lua`: damage handling (`hit_`), magic
/// (`mag_`), summons and sequence calls (`cal_`), bind end (`bck_`), hooks (`hck_`), attack
/// handling (`ahp_`, `aim_`), movement (`mch_`), shapes (`shp_`), Machinedramon's whip
/// triggers (`wh1_`, `wh2_`) and Tortamon's spin (`rot_`: its `swon`/`swof` sequence calls
/// record the spinning actors that `hit_e_torta.lua` makes ignore hits). The boss-only `ded_`
/// and `wht_` scripts are left behind: no donor is a boss.
pub(super) const MODEL_SCRIPTS: [&str; 13] = [
    "hit_", "mag_", "cal_", "bck_", "hck_", "ahp_", "aim_", "mch_", "shp_", "wh1_", "wh2_", "bht_",
    "rot_",
];
/// Classes of the effect infos a stage registers by name for the attacks of its enemies
/// (`x_026eblst01a`, `y_027emagc01a`, ...): the starter that delivers the attack and its
/// particles. An enemy's attack looks them up by name, so a copied model needs them too.
const EFFECT_CLASSES: [&str; 3] = ["beActionStarterInfo", "beParticleCtrl2Info", "ParticleInfo"];

/// The named effect infos in the pack's root `igInfoList`: (class, name, id).
fn effect_infos(p: &Pack) -> Result<Vec<(String, String, u32)>> {
    let root = u32at(p.b, p.table + 8 + 2 * p.ids.len())?;
    p.class(root, "igInfoList")?;
    let mut out = Vec::new();
    for id in p.array(root)? {
        let r = p.rec(id)?;
        let class = &p.classes[r.class].name;
        if !EFFECT_CLASSES.contains(&class.as_str()) {
            continue;
        }
        if let Some(name) = name_of(p, r).map(|n| String::from_utf8_lossy(n).into_owned()) {
            if !name.is_empty() {
                out.push((class.clone(), name, id));
            }
        }
    }
    Ok(out)
}

/// Lists whose memory block holds plain values rather than object IDs.
const RAW_LISTS: [&str; 6] = [
    "igVec3fList",
    "igQuaternionfList",
    "igLongList",
    "igIntList",
    "igUnsignedShortList",
    "ParticleArray",
];

/// Rename whole string constants in the named `beLuaData` scripts of one AFS entry.
pub struct LuaEdit {
    pub entry: usize,
    pub scripts: Vec<String>,
    pub names: BTreeMap<String, String>,
}

/// What to transplant where, and which generator rows to retarget.
pub struct Spec {
    pub model_entry: usize,
    pub source_entry: usize,
    pub models: Vec<String>,
    /// Further `(source entry, models)` grafts applied after the first.
    pub extra_sources: Vec<(usize, Vec<String>)>,
    pub generator_entries: Vec<usize>,
    pub rows: BTreeMap<u32, u32>,
    /// Per-model row maps: when set, each generator takes the map its name hashes to.
    pub spread: Vec<BTreeMap<u32, u32>>,
    /// When not empty, only generators with these names are retargeted.
    pub only_generators: Vec<String>,
    /// Generators (by name) that take every spawn from the named donor model instead,
    /// keeping their position inside the model family so area/difficulty tiers carry over.
    pub generator_models: BTreeMap<String, String>,
    /// String-constant renames inside stage Lua.
    pub lua_edits: Vec<LuaEdit>,
    /// Keep the Crown expansion of the destination model pack (the shipped behaviour).
    pub expand_model_pack: bool,
}

impl Spec {
    /// Abyss of Grief (`stag0102`): Goburimon becomes Mummymon, copied from Mummy Desert.
    /// Rows keep their position inside the model family, so area/difficulty tiers carry over.
    pub fn abyss_mummy() -> Self {
        let mut rows = BTreeMap::new();
        for k in 0..12 {
            rows.insert(k, 114 + k);
        }
        for k in 0..15 {
            rows.insert(379 + k, 394 + k);
        }
        Self {
            model_entry: 621,
            source_entry: 642,
            models: vec!["e_mummy".into()],
            extra_sources: Vec::new(),
            generator_entries: vec![622, 623, 624],
            rows,
            spread: Vec::new(),
            only_generators: Vec::new(),
            generator_models: BTreeMap::new(),
            lua_edits: Vec::new(),
            expand_model_pack: true,
        }
    }
}

impl Spec {
    /// Control for the graft: every Goburimon row becomes the Ogremon row already carried by
    /// the stage, with no model copied.
    pub fn abyss_ogre() -> Self {
        let mut spec = Self::abyss_mummy();
        spec.models.clear();
        for v in spec.rows.values_mut() {
            *v = 36;
        }
        spec
    }
}

impl Spec {
    /// Graft into the unexpanded model pack, to separate graft faults from crown expansion.
    pub fn abyss_mummy_plain() -> Self {
        Self {
            expand_model_pack: false,
            ..Self::abyss_mummy()
        }
    }
}

impl Spec {
    /// Memory stress test: graft every `(source entry, model)` into Abyss of Grief and spread
    /// all of its Goburimon, Blossomon and Ogremon spawns across them, keeping each row's
    /// position inside its own family so the tiers roughly carry over.
    pub fn abyss_stress(models: &[(usize, &str)]) -> Self {
        Self::abyss_stress_families(models, &["e_goburi", "e_blosso", "e_ogre"])
    }

    /// As [`Self::abyss_stress`], retargeting only the spawns of the listed families.
    pub fn abyss_stress_families(models: &[(usize, &str)], families: &[&str]) -> Self {
        let mut spec = Self::abyss_mummy();
        spec.rows.clear();
        let first = models[0].0;
        spec.source_entry = first;
        spec.models.clear();
        for &(entry, model) in models {
            let name = model.to_string();
            if entry == first {
                spec.models.push(name);
            } else if let Some(g) = spec.extra_sources.iter_mut().find(|g| g.0 == entry) {
                g.1.push(name);
            } else {
                spec.extra_sources.push((entry, vec![name]));
            }
        }
        let group = |m: &str| crate::rowgroup::ROW_GROUPS.iter().find(|g| g.model == m);
        for &(_, target) in models {
            let Some(to) = group(target) else { continue };
            let mut map = BTreeMap::new();
            for &own in families {
                let Some(from) = group(own) else { continue };
                for (k, &row) in from.rows.iter().enumerate() {
                    map.insert(row as u32, to.rows[k.min(to.rows.len() - 1)] as u32);
                }
            }
            spec.spread.push(map);
        }
        spec
    }
}

/// What a graft added, for reporting.
#[derive(Debug, Default, Clone)]
pub struct Report {
    pub objects: usize,
    pub blocks: usize,
    pub object_bytes: usize,
    pub block_bytes: usize,
    pub registered: usize,
    pub scripts: usize,
    /// Effect and bullet switches the destination's effect tree had no matching node for.
    pub unattached: Vec<String>,
    /// Own effects hung in the nearest matching node of a differently shaped effect tree.
    pub approximate: Vec<String>,
}

/// Memory blocks that are tables of IDs: the position, normal and UV stream blocks of a mesh.
/// (The strip-length block next to it is plain data: its one word is a vertex count.)
fn id_table(class: &str, slot: u16) -> bool {
    matches!((class, slot), ("igVertexArray1_1", 2))
}

fn block_words(p: &Pack, block: u32) -> Result<Vec<u32>> {
    let r = p
        .memory
        .get(block as usize)
        .and_then(|x| x.as_ref())
        .ok_or_else(|| bad("ID table is not a memory block"))?;
    (0..r.len() / 4)
        .map(|i| u32at(p.b, r.start + 4 * i))
        .collect()
}

fn object_list(name: &str) -> bool {
    name.ends_with("List") && !RAW_LISTS.contains(&name)
}

/// Object-reference (`false`) and memory-reference (`true`) slots of a record's class.
pub(super) fn ref_slots<'x>(p: &'x Pack, r: &Record) -> impl Iterator<Item = (u16, bool)> + 'x {
    p.classes[r.class]
        .fields
        .iter()
        .filter_map(|&(t, n, _)| match t {
            0 => Some((n, false)),
            7 => Some((n, true)),
            _ => None,
        })
}

pub(super) fn name_of<'x>(p: &'x Pack, r: &Record) -> Option<&'x [u8]> {
    let f = r.field(2).ok()?;
    p.b.get(f.start + 4..f.end)?.split(|c| *c == 0).next()
}

pub(super) fn find_named(p: &Pack, class: &str, name: &str) -> Result<Option<u32>> {
    let hits = (0..p.ids.len() as u32)
        .filter(|&id| {
            p.ids[id as usize].is_some_and(|i| {
                let r = &p.objects[i];
                p.classes[r.class].name == class && name_of(p, r) == Some(name.as_bytes())
            })
        })
        .collect::<Vec<_>>();
    match hits.len() {
        0 => Ok(None),
        1 => Ok(Some(hits[0])),
        _ => Err(bad(&format!("several {class} records named {name}"))),
    }
}

fn list_elements(p: &Pack, owner: &Record, block: u32) -> Result<Vec<u32>> {
    let count = owner.val(p.b, 2)? as usize;
    let r = p
        .memory
        .get(block as usize)
        .and_then(|x| x.as_ref())
        .ok_or_else(|| bad("list memory is not a memory block"))?;
    need(count <= r.len() / 4, "list exceeds its memory block")?;
    (0..count).map(|i| u32at(p.b, r.start + 4 * i)).collect()
}

/// Every object and memory block reachable from `roots`, plus every memory block that is a
/// table of IDs (object lists and mesh stream tables) with how many leading words are IDs.
pub(super) fn closure(p: &Pack, roots: &[u32]) -> Result<(BTreeSet<u32>, HashMap<u32, usize>)> {
    let _timing = crate::profiling::measure("graft.closure");
    let mut seen = BTreeSet::new();
    let mut tables = HashMap::new();
    let mut queue = roots.to_vec();
    while let Some(id) = queue.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(idx) = p
            .ids
            .get(id as usize)
            .copied()
            .ok_or_else(|| bad("ID out of range"))?
        else {
            need(
                p.memory[id as usize].is_some(),
                "reference to an unknown ID",
            )?;
            continue;
        };
        let r = &p.objects[idx];
        let class = &p.classes[r.class].name;
        let list = object_list(class);
        for (n, memory) in ref_slots(p, r) {
            let v = r.val(p.b, n)?;
            if v == NULL {
                continue;
            }
            let known = p.ids.get(v as usize).is_some_and(|x| x.is_some())
                || p.memory.get(v as usize).is_some_and(|x| x.is_some());
            need(known, "dangling reference")?;
            queue.push(v);
            if memory && (list || id_table(class, n)) {
                let elements = if list {
                    list_elements(p, r, v)?
                } else {
                    block_words(p, v)?
                };
                if tables.insert(v, elements.len()).is_some() {
                    return Err(bad("ID table shared between owners"));
                }
                for e in elements {
                    if e == NULL {
                        continue;
                    }
                    let object = p.ids.get(e as usize).is_some_and(|x| x.is_some());
                    let block = p.memory.get(e as usize).is_some_and(|x| x.is_some());
                    need(
                        if list { object } else { object || block },
                        "ID table entry is not a known ID",
                    )?;
                    queue.push(e);
                }
            }
        }
    }
    Ok((seen, tables))
}

/// Refuse a copy when a block that is not an ID table still looks like one: its words are
/// IDs of things outside the copy, which would silently point at unrelated destination data.
pub(super) fn check_blocks(
    p: &Pack,
    ids: &BTreeSet<u32>,
    tables: &HashMap<u32, usize>,
) -> Result<()> {
    // Bit masks hold flags, which can coincide with record IDs.
    let mut flags = BTreeSet::new();
    for &id in ids {
        let Some(idx) = p.ids.get(id as usize).copied().flatten() else {
            continue;
        };
        let r = &p.objects[idx];
        if p.classes[r.class].name == "igBitMask" {
            for (n, memory) in ref_slots(p, r) {
                if memory {
                    flags.insert(r.val(p.b, n)?);
                }
            }
        }
    }
    for &id in ids {
        if flags.contains(&id) {
            continue;
        }
        let Some(range) = p.memory.get(id as usize).and_then(|x| x.as_ref()) else {
            continue;
        };
        if tables.contains_key(&id) {
            continue;
        }
        let words = (0..range.len() / 4)
            .map(|i| u32at(p.b, range.start + 4 * i))
            .collect::<Result<Vec<_>>>()?;
        let used = words
            .iter()
            .copied()
            .filter(|&w| w != 0 && w != NULL)
            .collect::<Vec<_>>();
        let known = |w: u32| {
            p.ids.get(w as usize).is_some_and(|x| x.is_some())
                || p.memory.get(w as usize).is_some_and(|x| x.is_some())
        };
        // Packed data may hold a stray ID-like word; a real ID table is ID-like throughout.
        if !used.is_empty()
            && used.iter().all(|&w| w >= 4096 && known(w))
            && used.iter().any(|w| !ids.contains(w))
        {
            return Err(bad(&format!(
                "block {id} looks like an ID table outside the copy"
            )));
        }
    }
    Ok(())
}

/// The `igGroup` whose child list holds `child`: (group, child list ID).
pub(super) fn parent_group(p: &Pack, child: u32) -> Result<(u32, u32)> {
    let mut found = Vec::new();
    for id in 0..p.ids.len() as u32 {
        let Some(idx) = p.ids[id as usize] else {
            continue;
        };
        let r = &p.objects[idx];
        if p.classes[r.class].name != "igGroup" {
            continue;
        }
        for (n, memory) in ref_slots(p, r) {
            if memory {
                continue;
            }
            let v = r.val(p.b, n)?;
            if v != NULL && p.class(v, "igNodeList").is_ok() && p.array(v)?.contains(&child) {
                found.push((id, v));
            }
        }
    }
    need(
        found.len() == 1,
        "expected one scene-graph parent for the model switch",
    )?;
    Ok(found[0])
}

/// The child list of the unique `igGroup` called `name`.
fn group_children(p: &Pack, name: &[u8]) -> Result<u32> {
    let groups = (0..p.ids.len() as u32)
        .filter(|&id| {
            p.ids[id as usize].is_some_and(|i| {
                let r = &p.objects[i];
                p.classes[r.class].name == "igGroup" && name_of(p, r) == Some(name)
            })
        })
        .collect::<Vec<_>>();
    need(groups.len() == 1, "expected one destination scene group")?;
    let r = p.rec(groups[0])?;
    let lists = ref_slots(p, r)
        .filter(|&(_, memory)| !memory)
        .filter_map(|(n, _)| r.val(p.b, n).ok())
        .filter(|&v| v != NULL && p.class(v, "igNodeList").is_ok())
        .collect::<Vec<_>>();
    need(
        lists.len() == 1,
        "expected one child list on the scene group",
    )?;
    Ok(lists[0])
}

fn class_index(dp: &Pack, sp: &Pack, class: usize) -> Result<usize> {
    let c = &sp.classes[class];
    let i = dp
        .classes
        .iter()
        .position(|d| d.name == c.name)
        .ok_or_else(|| bad(&format!("destination pack lacks class {}", c.name)))?;
    need(
        dp.classes[i].fields == c.fields,
        &format!("class {} differs between packs", c.name),
    )?;
    Ok(i)
}

/// How a stage hangs a model in the scene: its switch under a shared enemy group (most
/// stages), or as the root of the model's own `igSceneInfo` (stages with one model).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ModelScene {
    Group(Vec<u8>),
    Info(u32),
}

pub(super) fn model_scene(sp: &Pack, model: &str) -> Result<ModelScene> {
    let info =
        find_named(sp, "beAction2Info", model)?.ok_or_else(|| bad("source lacks action info"))?;
    let switch = sp.val(info, 7)?;
    sp.class(switch, "igSwitch")?;
    if let Ok((group, _)) = parent_group(sp, switch) {
        let name = name_of(sp, sp.rec(group)?).ok_or_else(|| bad("unnamed scene group"))?;
        return Ok(ModelScene::Group(name.to_vec()));
    }
    let id =
        find_named(sp, "igSceneInfo", model)?.ok_or_else(|| bad("model switch has no scene"))?;
    Ok(ModelScene::Info(id))
}

/// Actual draw ancestry, rather than resource-reference reachability: an action
/// info can retain a model switch even after its scene registration is removed.
pub(super) fn scene_nodes(p: &Pack, scene: u32) -> Result<BTreeSet<u32>> {
    p.class(scene, "igSceneInfo")?;
    let mut queue = vec![p.val(scene, 5)?];
    let additional = p.val(scene, 11)?;
    if additional != NULL {
        queue.extend(p.array(additional)?);
    }
    let mut nodes = BTreeSet::new();
    while let Some(id) = queue.pop() {
        if id == NULL || !nodes.insert(id) {
            continue;
        }
        let record = p.rec(id)?;
        if !p.classes[record.class]
            .fields
            .iter()
            .any(|&(kind, field, _)| kind == 0 && field == 7)
        {
            continue;
        }
        let children = p.val(id, 7)?;
        if children != NULL && p.class(children, "igNodeList").is_ok() {
            queue.extend(p.array(children)?);
        }
    }
    Ok(nodes)
}

pub(super) fn model_is_drawn(p: &Pack, model: &str) -> Result<bool> {
    let action = p.named("beAction2Info", model)?;
    let switch = p.val(action, 7)?;
    let registry = u32at(p.b, p.table + 8 + 2 * p.ids.len())?;
    for info in p.array(registry)? {
        if p.class(info, "igSceneInfo").is_ok() && scene_nodes(p, info)?.contains(&switch) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The effect number each enemy model owns: its bullets and magic are `x_NNNe...` models and
/// `y_NNNe...` effects (Infermon is 026). Read off which models every stage using the number
/// carries (`probe_numbers`).
pub(super) const EFFECT_NUMBERS: [(&str, &str); 30] = [
    ("e_goburi", "001"),
    ("e_cockatri", "002"),
    ("e_nume", "003"),
    ("e_ogre", "004"),
    ("e_scum", "005"),
    ("e_blosso", "006"),
    ("e_mammos", "007"),
    ("e_mecha1", "008"),
    ("e_arachne", "009"),
    ("e_tyrano", "010"),
    ("e_mummy", "011"),
    ("e_minotaru", "012"),
    ("e_pharaoh", "013"),
    ("e_mecha2", "014"),
    ("e_anomalo", "016"),
    ("e_otama", "018"),
    ("e_geko", "019"),
    ("e_maridevi", "020"),
    ("e_mecha3", "021"),
    ("e_tonogeko", "022"),
    ("e_diabolo", "024"),
    ("e_impdra", "025"),
    ("e_infermo", "026"),
    ("e_megadra", "027"),
    ("e_rare", "028"),
    ("e_mecha5", "029"),
    ("e_ldknight", "032"),
    ("e_skulgrey", "015"),
    ("e_torta", "017"),
    ("e_mecha4", "023"),
];

/// Enemy projectiles (`x_026eblst01a`, ...) are models of their own with their own hit data,
/// registered by the stage that has an enemy using them. The bullet models of `models` that
/// `sp` registers and `dp` (when given) lacks.
pub(super) fn companion_models(
    sp: &Pack,
    dp: Option<&dyn Destination>,
    models: &[String],
) -> Result<Vec<String>> {
    let prefixes: Vec<String> = models
        .iter()
        .filter_map(|m| EFFECT_NUMBERS.iter().find(|(n, _)| n == m))
        .map(|(_, number)| format!("x_{number}e"))
        .collect();
    let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len())?;
    sp.class(root, "igInfoList")?;
    let mut out = BTreeSet::new();
    for id in sp.array(root)? {
        let r = sp.rec(id)?;
        if sp.classes[r.class].name != "beModelCtrlInfo" {
            continue;
        }
        let Some(name) = name_of(sp, r).map(|n| String::from_utf8_lossy(n).into_owned()) else {
            continue;
        };
        if prefixes.iter().any(|p| name.starts_with(p.as_str()))
            && dp.is_none_or(|d| d.named("beModelCtrlInfo", &name).ok().flatten().is_none())
        {
            out.insert(name);
        }
    }
    Ok(out.into_iter().collect())
}

/// One effect's native render ancestry. Each edge is (parent, child list, selected child),
/// walking from its switch up to the named effect pass.
struct Attach {
    unit: u32,
    top: u32,
    path: Vec<(u32, u32, u32)>,
}

/// The nodes whose child list holds `child`: (node, child list).
#[cfg(test)]
fn node_holders(p: &Pack, child: u32) -> Vec<(u32, u32)> {
    let mut found = Vec::new();
    for id in 0..p.ids.len() as u32 {
        let Some(idx) = p.ids[id as usize] else {
            continue;
        };
        let r = &p.objects[idx];
        for (n, memory) in ref_slots(p, r) {
            if memory {
                continue;
            }
            let Ok(v) = r.val(p.b, n) else { continue };
            if v != NULL
                && p.class(v, "igNodeList").is_ok()
                && p.array(v).is_ok_and(|a| a.contains(&child))
            {
                found.push((id, v));
            }
        }
    }
    found
}

/// The destination lookups that decide what a graft copies. A real pack answers
/// them from its records; the analysis destination answers them from a summary
/// that each planned graft updates, without copying any bytes.
pub(super) trait Destination {
    /// `find_named`: none, one (an ID) or an error for several.
    fn named(&self, class: &str, name: &str) -> Result<Option<u32>>;
    /// `group_children`: the child list of the unique group with this name.
    fn group(&self, name: &[u8]) -> Result<u32>;
    /// Whether an effect info with this class and name is registered.
    fn has_effect(&self, class: &str, name: &str) -> Result<bool>;
    /// The child list of the unique unparented node with this class and name.
    fn pass(&self, class: &str, name: &[u8]) -> Result<Option<u32>>;
    fn classes(&self) -> &[Class];
}

pub(super) struct PackDestination<'p, 'a> {
    p: &'p Pack<'a>,
    index: std::cell::OnceCell<SceneIndex>,
    effects: std::cell::OnceCell<BTreeSet<(String, String)>>,
}

impl<'p, 'a> PackDestination<'p, 'a> {
    pub(super) fn new(p: &'p Pack<'a>) -> Self {
        Self {
            p,
            index: std::cell::OnceCell::new(),
            effects: std::cell::OnceCell::new(),
        }
    }

    fn effect_keys(&self) -> Result<&BTreeSet<(String, String)>> {
        if self.effects.get().is_none() {
            let keys = effect_infos(self.p)?
                .into_iter()
                .map(|(c, n, _)| (c, n))
                .collect();
            let _ = self.effects.set(keys);
        }
        Ok(self.effects.get().unwrap())
    }
}

impl Destination for PackDestination<'_, '_> {
    fn named(&self, class: &str, name: &str) -> Result<Option<u32>> {
        find_named(self.p, class, name)
    }
    fn group(&self, name: &[u8]) -> Result<u32> {
        group_children(self.p, name)
    }
    fn has_effect(&self, class: &str, name: &str) -> Result<bool> {
        Ok(self
            .effect_keys()?
            .contains(&(class.to_string(), name.to_string())))
    }
    fn pass(&self, class: &str, name: &[u8]) -> Result<Option<u32>> {
        effect_pass(
            self.p,
            class,
            name,
            self.index.get_or_init(|| SceneIndex::new(self.p)),
        )
    }
    fn classes(&self) -> &[Class] {
        &self.p.classes
    }
}

/// The single scene child list of a node, as `group_children` and
/// `node_children` require it.
fn one_child_list(p: &Pack, id: u32) -> Option<u32> {
    let r = p.rec(id).ok()?;
    let lists = ref_slots(p, r)
        .filter(|&(_, memory)| !memory)
        .filter_map(|(n, _)| r.val(p.b, n).ok())
        .filter(|&v| v != NULL && p.class(v, "igNodeList").is_ok())
        .collect::<Vec<_>>();
    (lists.len() == 1).then(|| lists[0])
}

#[derive(Clone)]
struct NamedNode {
    list: Option<u32>,
    unheld: bool,
}

/// Analysis destination: the facts [`Destination`] needs about a stage pack,
/// updated by each planned graft as the copied records would update the pack.
#[derive(Clone)]
pub(super) struct VirtualDestination {
    ids: usize,
    names: HashMap<(String, Vec<u8>), Vec<NamedNode>>,
    counts: HashMap<String, usize>,
    effects: BTreeSet<(String, String)>,
    classes: Vec<Class>,
}

impl VirtualDestination {
    pub(super) fn new(p: &Pack) -> Result<Self> {
        let index = SceneIndex::new(p);
        let mut me = Self {
            ids: p.ids.len(),
            names: HashMap::new(),
            counts: HashMap::new(),
            effects: PackDestination::new(p).effect_keys()?.clone(),
            classes: p.classes.clone(),
        };
        for id in 0..p.ids.len() as u32 {
            let unheld = index.holders(id).is_empty();
            me.record(p, id, unheld);
        }
        Ok(me)
    }

    fn record(&mut self, p: &Pack, id: u32, unheld: bool) {
        let Some(index) = p.ids[id as usize] else {
            return;
        };
        let r = &p.objects[index];
        let class = &p.classes[r.class].name;
        *self.counts.entry(class.clone()).or_default() += 1;
        if let Some(name) = name_of(p, r) {
            self.names
                .entry((class.clone(), name.to_vec()))
                .or_default()
                .push(NamedNode {
                    list: one_child_list(p, id),
                    unheld,
                });
        }
    }

    pub(super) fn id_count(&self) -> usize {
        self.ids
    }

    /// Define classes as `add_classes` does before a successful graft.
    fn add_classes(&mut self, defs: &[ClassDef]) {
        self.classes.extend(defs.iter().map(|d| d.class.clone()));
    }

    fn nodes(&self, class: &str, name: &[u8]) -> &[NamedNode] {
        self.names
            .get(&(class.to_string(), name.to_vec()))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

impl Destination for VirtualDestination {
    fn named(&self, class: &str, name: &str) -> Result<Option<u32>> {
        match self.nodes(class, name.as_bytes()).len() {
            0 => Ok(None),
            1 => Ok(Some(0)),
            _ => Err(bad(&format!("several {class} records named {name}"))),
        }
    }
    fn group(&self, name: &[u8]) -> Result<u32> {
        let groups = self.nodes("igGroup", name);
        need(groups.len() == 1, "expected one destination scene group")?;
        groups[0]
            .list
            .ok_or_else(|| bad("expected one child list on the scene group"))
    }
    fn has_effect(&self, class: &str, name: &str) -> Result<bool> {
        Ok(self
            .effects
            .contains(&(class.to_string(), name.to_string())))
    }
    fn pass(&self, class: &str, name: &[u8]) -> Result<Option<u32>> {
        let tops = self
            .nodes(class, name)
            .iter()
            .filter(|n| n.unheld)
            .collect::<Vec<_>>();
        if tops.len() != 1 {
            return Ok(None);
        }
        tops[0]
            .list
            .map(Some)
            .ok_or_else(|| bad("expected one effect child list"))
    }
    fn classes(&self) -> &[Class] {
        &self.classes
    }
}

/// One destination lookup made while planning a copy, with its answer.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Lookup {
    Named(String, String, std::result::Result<Option<u32>, String>),
    Group(Vec<u8>, std::result::Result<u32, String>),
    Effect(String, String, std::result::Result<bool, String>),
    Pass(String, Vec<u8>, std::result::Result<Option<u32>, String>),
}

/// The planning code uses only whether a record or list exists, never which
/// destination ID it has, so answers record IDs as 0.
fn answer<T>(r: &Result<T>, shape: impl Fn(&T) -> T) -> std::result::Result<T, String> {
    r.as_ref().map(shape).map_err(ToString::to_string)
}

fn some(o: &Option<u32>) -> Option<u32> {
    o.map(|_| 0)
}

fn zero(_: &u32) -> u32 {
    0
}

fn same(b: &bool) -> bool {
    *b
}

/// Records every lookup the planning code makes, in order.
struct Recording<'d> {
    inner: &'d dyn Destination,
    log: std::cell::RefCell<Vec<Lookup>>,
}

impl Destination for Recording<'_> {
    fn named(&self, class: &str, name: &str) -> Result<Option<u32>> {
        let r = self.inner.named(class, name);
        self.log
            .borrow_mut()
            .push(Lookup::Named(class.into(), name.into(), answer(&r, some)));
        r
    }
    fn group(&self, name: &[u8]) -> Result<u32> {
        let r = self.inner.group(name);
        self.log
            .borrow_mut()
            .push(Lookup::Group(name.to_vec(), answer(&r, zero)));
        r
    }
    fn has_effect(&self, class: &str, name: &str) -> Result<bool> {
        let r = self.inner.has_effect(class, name);
        self.log
            .borrow_mut()
            .push(Lookup::Effect(class.into(), name.into(), answer(&r, same)));
        r
    }
    fn pass(&self, class: &str, name: &[u8]) -> Result<Option<u32>> {
        let r = self.inner.pass(class, name);
        self.log
            .borrow_mut()
            .push(Lookup::Pass(class.into(), name.to_vec(), answer(&r, some)));
        r
    }
    fn classes(&self) -> &[Class] {
        self.inner.classes()
    }
}

impl Lookup {
    /// Whether `dest` still gives the recorded answer.
    fn holds(&self, dest: &dyn Destination) -> bool {
        match self {
            Lookup::Named(c, n, a) => answer(&dest.named(c, n), some) == *a,
            Lookup::Group(n, a) => answer(&dest.group(n), zero) == *a,
            Lookup::Effect(c, n, a) => answer(&dest.has_effect(c, n), same) == *a,
            Lookup::Pass(c, n, a) => answer(&dest.pass(c, n), some) == *a,
        }
    }
}

/// What a planned copy adds to the destination summary.
#[derive(Clone)]
struct Delta {
    ids: usize,
    scripts: bool,
    records: Vec<(String, Option<Vec<u8>>, NamedNode)>,
    effects: Vec<(String, String)>,
}

type Variants = Vec<(Vec<Lookup>, std::result::Result<Delta, String>)>;
type PlanKey = ([u8; 16], Vec<String>);

/// Planned copies by (source pack contents, models), each valid for the recorded
/// lookups. Plans depend only on those, so analysis runs share one cache.
static VIRTUAL_PLANS: std::sync::LazyLock<
    std::sync::Mutex<HashMap<PlanKey, std::sync::Arc<std::sync::RwLock<Variants>>>>,
> = std::sync::LazyLock::new(Default::default);

/// The content fingerprint of a donor source pack.
pub(super) fn source_fingerprint(src: &[u8]) -> [u8; 16] {
    use md5::{Digest, Md5};
    Md5::digest(src).into()
}

fn plan_delta(dest: &dyn Destination, src: &[u8], models: &[String]) -> Result<Delta> {
    let original = Pack::parse(src)?;
    let RenderSource {
        bytes: rendered,
        attached,
    } = render_source(&original, Some(dest), models, &mut Report::default())
        .map_err(|e| bad(&format!("render source: {e}")))?;
    let sp = Pack::parse(&rendered)?;
    let CopyPlan {
        infos,
        scripts,
        all,
        ids,
        ..
    } = copy_plan(dest, &sp, &attached, models, &MODEL_SCRIPTS)?;
    // Copied nodes the destination's own lists will hold: effect tops join a
    // render pass and model switches join their scene group.
    let mut held: BTreeSet<u32> = attached.iter().map(|&(top, _)| top).collect();
    for (m, strict) in &all {
        let scene = if *strict {
            Some(model_scene(&sp, m)?)
        } else {
            model_scene(&sp, m).ok()
        };
        let Some(ModelScene::Group(name)) = scene else {
            continue;
        };
        if !*strict && dest.group(&name).is_err() {
            continue;
        }
        let info =
            find_named(&sp, "beAction2Info", m)?.ok_or_else(|| bad("source lacks action info"))?;
        dest.group(&name)?;
        held.insert(sp.val(info, 7)?);
    }
    let index = SceneIndex::new(&sp);
    let mut records = Vec::new();
    for &id in &ids {
        let Some(i) = sp.ids[id as usize] else {
            continue;
        };
        let r = &sp.objects[i];
        let unheld = !held.contains(&id)
            && !index
                .holders(id)
                .iter()
                .any(|(owner, _)| ids.contains(owner));
        records.push((
            sp.classes[r.class].name.clone(),
            name_of(&sp, r).map(<[u8]>::to_vec),
            NamedNode {
                list: one_child_list(&sp, id),
                unheld,
            },
        ));
    }
    let mut effects = Vec::new();
    for id in infos {
        let r = sp.rec(id)?;
        let class = &sp.classes[r.class].name;
        if EFFECT_CLASSES.contains(&class.as_str()) {
            if let Some(name) = name_of(&sp, r).filter(|n| !n.is_empty()) {
                effects.push((class.clone(), String::from_utf8_lossy(name).into_owned()));
            }
        }
    }
    Ok(Delta {
        ids: ids.len(),
        scripts: !scripts.is_empty(),
        records,
        effects,
    })
}

/// Plan [`graft`] against an analysis destination: the same checks and the same
/// copy, with no bytes written. Returns how many IDs the copy adds.
pub(super) fn virtual_graft(
    dest: &mut VirtualDestination,
    source: ([u8; 16], &[u8]),
    models: &[String],
    missing: &[ClassDef],
) -> Result<usize> {
    let _timing = crate::profiling::measure("graft.virtual");
    let variants = VIRTUAL_PLANS
        .lock()
        .map_err(|_| bad("virtual plan cache poisoned"))?
        .entry((source.0, models.to_vec()))
        .or_default()
        .clone();
    let found = variants
        .read()
        .map_err(|_| bad("virtual plan cache poisoned"))?
        .iter()
        .find(|(lookups, _)| lookups.iter().all(|l| l.holds(&*dest)))
        .map(|(_, delta)| delta.clone());
    let delta = match found {
        Some(delta) => delta,
        None => {
            let _miss = crate::profiling::measure("graft.virtual.miss");
            let recording = Recording {
                inner: &*dest,
                log: Default::default(),
            };
            let delta = plan_delta(&recording, source.1, models).map_err(|e| e.to_string());
            variants
                .write()
                .map_err(|_| bad("virtual plan cache poisoned"))?
                .push((recording.log.into_inner(), delta.clone()));
            delta
        }
    }
    .map_err(|e| bad(&e))?;
    need(
        dest.ids + delta.ids < u16::MAX as usize,
        &format!(
            "copy resource graph: grafted pack exceeds 16-bit directory index ({} objects, {} more needed, {} allowed)",
            dest.ids,
            delta.ids,
            u16::MAX
        ),
    )?;
    if delta.scripts {
        need(
            dest.counts.get("beLuaDataList").copied() == Some(1),
            "expected one script list",
        )?;
    }
    for (class, name, node) in delta.records {
        *dest.counts.entry(class.clone()).or_default() += 1;
        if let Some(name) = name {
            dest.names.entry((class, name)).or_default().push(node);
        }
    }
    dest.effects.extend(delta.effects);
    // `Builder::finish` pads an odd number of new IDs with one NULL block.
    let added = delta.ids + delta.ids % 2;
    dest.ids += added;
    dest.add_classes(missing);
    Ok(added)
}

/// Reverse the scene's child lists once per immutable pack view. Looking up
/// every ancestor by rescanning every object repeats the same work for each
/// effect and for each level of its render ancestry.
struct SceneIndex {
    holders: BTreeMap<u32, Vec<(u32, u32)>>,
}
impl SceneIndex {
    fn new(p: &Pack) -> Self {
        let _timing = crate::profiling::measure("graft.scene_index");
        let mut holders: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
        for (id, index) in p.ids.iter().enumerate() {
            let Some(index) = index else { continue };
            let r = &p.objects[*index];
            for (slot, memory) in ref_slots(p, r) {
                if memory {
                    continue;
                }
                let Ok(list) = r.val(p.b, slot) else { continue };
                if list == NULL || p.class(list, "igNodeList").is_err() {
                    continue;
                }
                let Ok(children) = p.array(list) else {
                    continue;
                };
                // The old lookup used contains(): duplicate children count once
                // per owner field, while duplicate owner fields remain distinct.
                for child in children.into_iter().collect::<BTreeSet<_>>() {
                    holders.entry(child).or_default().push((id as u32, list));
                }
            }
        }
        Self { holders }
    }
    fn holders(&self, child: u32) -> &[(u32, u32)] {
        self.holders
            .get(&child)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

fn is_promoted(p: &Pack, id: u32) -> bool {
    p.rec(id).is_ok_and(|r| {
        p.classes[r.class].name == "igAttrSet" && name_of(p, r) == Some(b"PromotedAttr")
    })
}

/// Climb from a model's switch to the effect pass. Shared/ambiguous ancestry is
/// refused: selecting an arbitrary parent can change the texture or blending.
fn attach_point(sp: &Pack, switch: u32, index: &SceneIndex) -> Result<Option<Attach>> {
    let mut chain = vec![switch];
    let mut path = Vec::new();
    loop {
        need(chain.len() < 16, "scene nesting too deep")?;
        let child = *chain.last().unwrap();
        let holders = index.holders(child);
        match holders.len() {
            0 => break,
            1 => {
                let (holder, list) = holders[0];
                path.push((holder, list, child));
                chain.push(holder);
            }
            _ => return Ok(None),
        }
    }
    let Some(i) = (1..chain.len()).find(|&i| is_promoted(sp, chain[i])) else {
        return Ok(None);
    };
    let top = *chain.last().unwrap();
    if !name_of(sp, sp.rec(top)?)
        .unwrap_or_default()
        .starts_with(b"EF0")
    {
        return Ok(None);
    }
    Ok(Some(Attach {
        unit: chain[i - 1],
        top,
        path,
    }))
}

fn node_children(p: &Pack, node: u32) -> Result<u32> {
    let r = p.rec(node)?;
    let lists = ref_slots(p, r)
        .filter(|&(_, memory)| !memory)
        .filter_map(|(n, _)| r.val(p.b, n).ok())
        .filter(|&v| v != NULL && p.class(v, "igNodeList").is_ok())
        .collect::<Vec<_>>();
    need(lists.len() == 1, "expected one effect child list")?;
    Ok(lists[0])
}

/// Find the same render pass, without guessing at child indexes inside it. The
/// imported branch supplies its own complete attribute stack below this node.
fn effect_pass(dp: &Pack, class: &str, name: &[u8], index: &SceneIndex) -> Result<Option<u32>> {
    let tops = (0..dp.ids.len() as u32)
        .filter(|&id| {
            dp.rec(id)
                .is_ok_and(|d| dp.classes[d.class].name == class && name_of(dp, d) == Some(name))
                && index.holders(id).is_empty()
        })
        .collect::<Vec<_>>();
    if tops.len() != 1 {
        return Ok(None);
    }
    Ok(Some(node_children(dp, tops[0])?))
}

struct RenderSource {
    bytes: Vec<u8>,
    attached: Vec<(u32, u32)>,
}

/// Prepare a private source view containing only the selected paths through the
/// shared effect tree. Copying the ancestors without pruning would also import
/// every sibling effect. Counts and null-filled tails keep those siblings out of
/// both graph traversal and the loader's reference tables.
fn render_source(
    sp: &Pack,
    dp: Option<&dyn Destination>,
    models: &[String],
    report: &mut Report,
) -> Result<RenderSource> {
    let bullets = companion_models(sp, dp, models)?;
    let source_index = SceneIndex::new(sp);
    let mut attached = BTreeMap::new();
    let mut selected: BTreeMap<u32, (u32, BTreeSet<u32>)> = BTreeMap::new();
    for (switch, own, name) in effect_switches(sp, dp, models, &bullets.iter().collect::<Vec<_>>())?
    {
        let Some(at) = attach_point(sp, switch, &source_index)? else {
            if own {
                report.unattached.push(format!("{name} (no tree)"));
            }
            continue;
        };
        let destination = if let Some(dp) = dp {
            let top = sp.rec(at.top)?;
            let Some(list) = dp.pass(
                &sp.classes[top.class].name,
                name_of(sp, top).unwrap_or_default(),
            )?
            else {
                if own {
                    report.unattached.push(format!("{name} (no pass)"));
                }
                continue;
            };
            list
        } else {
            NULL
        };
        attached.insert(at.top, destination);
        for (parent, list, child) in at.path {
            let (holder, children) = selected.entry(list).or_insert((parent, BTreeSet::new()));
            need(*holder == parent, "effect ancestry shares a child list")?;
            children.insert(child);
        }
    }
    let mut edits = Vec::new();
    let mut blocks = BTreeSet::new();
    for (list, (parent, keep)) in selected {
        let original = sp.array(list)?;
        let children: Vec<_> = original
            .iter()
            .copied()
            .filter(|id| keep.contains(id))
            .collect();
        need(
            children.len() == keep.len(),
            "effect child missing from source",
        )?;
        // A switch mask addresses children by index. Only attribute/group
        // branches can be compacted without changing visibility semantics.
        need(
            children.len() == original.len() || sp.class(parent, "igSwitch").is_err(),
            "cannot prune an indexed effect ancestor",
        )?;
        let r = sp.rec(list)?;
        let block = sp.val(list, 4)?;
        need(blocks.insert(block), "effect ancestry shares child memory")?;
        let range = sp.memory[block as usize]
            .clone()
            .ok_or_else(|| bad("missing effect children"))?;
        let mut bytes = vec![0xff; range.len()];
        let head = words(&children);
        bytes[..head.len()].copy_from_slice(&head);
        edits.push(Edit { range, bytes });
        for slot in [2, 3] {
            edits.push(Edit {
                range: r.field(slot)?.clone(),
                bytes: (children.len() as u32).to_le_bytes().to_vec(),
            });
        }
    }
    edits.sort_by_key(|e| e.range.start);
    Ok(RenderSource {
        bytes: materialize(sp.b, &edits)?,
        attached: attached.into_iter().collect(),
    })
}

/// The scene nodes of bullets and effects that must be copied with the models: the switches
/// of the companion bullet models and of the `beParticleCtrl2Info` effects missing from `dp`,
/// each flagged when it belongs to one of `models` (by effect number) rather than to another
/// enemy of the source stage.
fn effect_switches(
    sp: &Pack,
    dp: Option<&dyn Destination>,
    models: &[String],
    bullets: &[&String],
) -> Result<Vec<(u32, bool, String)>> {
    let numbers: Vec<String> = models
        .iter()
        .filter_map(|m| EFFECT_NUMBERS.iter().find(|(n, _)| n == m))
        .map(|(_, number)| format!("_{number}e"))
        .collect();
    let mut out = Vec::new();
    for m in bullets {
        if let Some(info) = find_named(sp, "beAction2Info", m)? {
            out.push((sp.val(info, 7)?, true, m.to_string()));
        }
    }
    for (class, name, id) in effect_infos(sp)? {
        if class == "beParticleCtrl2Info"
            && !dp.map_or(Ok(false), |d| d.has_effect(&class, &name))?
        {
            if let Ok(switch) = sp.val(id, 7) {
                out.push((
                    switch,
                    numbers.iter().any(|n| name.contains(n.as_str())),
                    name.clone(),
                ));
            }
        }
    }
    out.retain(|&(s, _, _)| s != NULL && sp.class(s, "igSwitch").is_ok());
    Ok(out)
}

/// Give a standalone enemy scene the ordinary shared enemy group. The native
/// switch remains a child of the same scene; later imports can join EM01 using
/// the normal graft path. Copy only the donor's plain group and an empty list,
/// never the donor's actors, world scene or their texture/animation resources.
pub(super) fn add_enemy_group(dst: &[u8], src: &[u8], native: &str) -> Result<Vec<u8>> {
    let dp = Pack::parse(dst)?;
    if group_children(&dp, b"EM01").is_ok() {
        return Ok(dst.to_vec());
    }
    let scene = dp.named("igSceneInfo", native)?;
    let switch = dp.val(dp.named("beAction2Info", native)?, 7)?;
    need(
        dp.val(scene, 5)? == switch,
        "native scene root is not its model switch",
    )?;
    need(
        dp.array(dp.val(scene, 11)?)?.is_empty(),
        "native scene has additional roots",
    )?;
    let source = Pack::parse(src)?;
    let group = source.named("igGroup", "EM01")?;
    let list = group_children(&source, b"EM01")?;
    let block = source.val(list, 4)?;
    // The temporary source keeps all old backing references neutral. Its new
    // zero-length list owns no sibling actors and can grow normally after copy.
    let mut prune = Builder::new(&source);
    let empty = prune.mem(Vec::new(), block)?;
    prune.modify(list, &[(2, 0), (3, 0), (4, empty)])?;
    let old = source.memory[block as usize]
        .as_ref()
        .ok_or_else(|| bad("missing enemy group block"))?;
    prune.rewrite_mem(block, words(&vec![NULL; old.len() / 4]))?;
    let pruned = materialize(src, &prune.finish(block)?)?;
    let sp = Pack::parse(&pruned)?;
    let (ids, owners) = closure(&sp, &[group])?;
    check_blocks(&sp, &ids, &owners)?;
    need(
        ids.iter().all(|&id| {
            sp.ids[id as usize].is_none_or(|index| {
                matches!(
                    sp.classes[sp.objects[index].class].name.as_str(),
                    "igGroup" | "igNodeList"
                )
            })
        }),
        "enemy scene template contains more than a plain group",
    )?;

    let mut builder = Builder::new(&dp);
    let map = copy_graph(
        &dp,
        &sp,
        &mut builder,
        &ids,
        &owners,
        &mut Report::default(),
    )?;
    let grows = builder.grown();
    let rewrites = builder.rewrites.clone();
    let registry = u32at(dp.b, dp.table + 8 + 2 * dp.ids.len())?;
    let out = materialize(dst, &builder.finish(dp.val(registry, 4)?)?)?;
    verify(&dp, &sp, &out, &map, &owners, &grows, &rewrites)?;
    // Attach the native switch only after the copied empty list has real IDs.
    let p = Pack::parse(&out)?;
    let mut builder = Builder::new(&p);
    let children = map[&list];
    let memory = p.val(children, 4)?;
    builder.grow_mem(memory, words(&[switch]))?;
    builder.modify(children, &[(2, 1), (3, 1)])?;
    builder.modify(scene, &[(5, map[&group])])?;
    let result = materialize(&out, &builder.finish(memory)?)?;
    let after = Pack::parse(&result)?;
    need(
        after.array(group_children(&after, b"EM01")?)? == vec![switch],
        "enemy scene registration failed",
    )?;
    need(
        after.val(scene, 5)? == map[&group],
        "enemy scene root differs",
    )?;
    Ok(result)
}

/// Copy the named models' record subgraphs, scripts included, from `src` into `dst`.
pub(super) fn graft(dst: &[u8], src: &[u8], models: &[String]) -> Result<(Vec<Edit>, Report)> {
    graft_with_scripts(dst, src, models, &MODEL_SCRIPTS)
}

/// Keep the list's backing ID while inserting at its logical end. Reclamation
/// can leave spare NULL words beyond the count; growing after those words would
/// publish NULL entries and hide the last imported objects from the engine.
fn write_list(p: &Pack, builder: &mut Builder, list: u32, entries: &[u32]) -> Result<()> {
    let block = p.val(list, 4)?;
    let range = p
        .memory
        .get(block as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing list memory"))?;
    let mut bytes = words(entries);
    bytes.resize(bytes.len().max(range.len()), 0xff);
    builder.rewrite_mem(block, bytes)?;
    builder.modify(
        list,
        &[(2, entries.len() as u32), (3, entries.len() as u32)],
    )
}

/// Explicit script policy for development grafts that also supply arena-owned
/// callbacks. Ordinary enemy imports retain their existing script policy.
pub(super) fn graft_with_scripts(
    dst: &[u8],
    src: &[u8],
    models: &[String],
    prefixes: &[&str],
) -> Result<(Vec<Edit>, Report)> {
    let dp = Pack::parse(dst)?;
    let _timing = crate::profiling::measure("graft.copy");
    let original = Pack::parse(src)?;
    let mut report = Report::default();
    let destination = PackDestination::new(&dp);
    let RenderSource {
        bytes: rendered,
        attached,
    } = render_source(&original, Some(&destination), models, &mut report)
        .map_err(|e| bad(&format!("render source: {e}")))?;
    let sp = Pack::parse(&rendered)?;
    let CopyPlan {
        infos,
        scripts,
        all,
        ids,
        owners,
    } = copy_plan(&destination, &sp, &attached, models, prefixes)?;
    let mut builder = if prefixes.contains(&"ded_") {
        Builder::new(&dp).reuse_directories()
    } else {
        Builder::new(&dp)
    };
    let map = copy_graph(&dp, &sp, &mut builder, &ids, &owners, &mut report)
        .map_err(|e| bad(&format!("copy resource graph: {e}")))?;

    // Register the models and scripts where the engine looks them up.
    let root = u32at(dp.b, dp.table + 8 + 2 * dp.ids.len())?;
    dp.class(root, "igInfoList")?;
    let mut list = dp.array(root)?;
    list.extend(infos.iter().map(|i| map[i]));
    report.registered = infos.len();
    // Grow the registry's own memory block in place. Giving the root list a new block makes
    // the engine keep the whole stage pack resident after the stage is left (about 6 MB
    // each time, so the next stage runs out of memory). Other reference-bearing
    // lists must retain their backing blocks too, including the script registry.
    write_list(&dp, &mut builder, root, &list)?;
    let template = dp.val(root, 4)?;

    if !scripts.is_empty() {
        let lists = (0..dp.ids.len() as u32)
            .filter(|&i| dp.class(i, "beLuaDataList").is_ok())
            .collect::<Vec<_>>();
        need(lists.len() == 1, "expected one script list")?;
        let lua = lists[0];
        let mut entries = dp.array(lua)?;
        for (name, id) in &scripts {
            let at = entries.partition_point(|&e| {
                dp.rec(e)
                    .ok()
                    .and_then(|r| name_of(&dp, r))
                    .is_some_and(|n| n < name.as_slice())
            });
            entries.insert(at, map[id]);
        }
        report.scripts = scripts.len();
        write_list(&dp, &mut builder, lua, &entries)?;
    }
    // Draw registration: the enemy model switches hang under a scene group of the stage.
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (m, strict) in &all {
        let (m, strict) = (m.as_str(), *strict);
        let scene = if strict {
            Some(model_scene(&sp, m)?)
        } else {
            model_scene(&sp, m).ok()
        };
        let Some(ModelScene::Group(name)) = scene else {
            continue;
        };
        if !strict && group_children(&dp, &name).is_err() {
            continue;
        }
        let info =
            find_named(&sp, "beAction2Info", m)?.ok_or_else(|| bad("source lacks action info"))?;
        let switch = sp.val(info, 7)?;
        children
            .entry(group_children(&dp, &name)?)
            .or_default()
            .push(map[&switch]);
    }
    for &(unit, list) in &attached {
        children.entry(list).or_default().push(map[&unit]);
    }
    for (list, added) in children {
        let mut entries = dp.array(list)?;
        entries.extend(&added);
        write_list(&dp, &mut builder, list, &entries)?;
    }
    let grows = builder.grown();
    let rewrites = builder.rewrites.clone();
    let edits = builder.finish(template)?;
    let out = materialize(dst, &edits)?;
    verify(&dp, &sp, &out, &map, &owners, &grows, &rewrites)
        .map_err(|e| bad(&format!("verify resource graph: {e}")))?;
    Ok((edits, report))
}

/// What a graft copies and registers, decided only by the source and these
/// destination lookups. The real graft and the analysis destination share it.
pub(super) struct CopyPlan {
    infos: Vec<u32>,
    scripts: Vec<(Vec<u8>, u32)>,
    all: Vec<(String, bool)>,
    ids: BTreeSet<u32>,
    owners: HashMap<u32, usize>,
}

fn copy_plan(
    dp: &dyn Destination,
    sp: &Pack,
    attached: &[(u32, u32)],
    models: &[String],
    prefixes: &[&str],
) -> Result<CopyPlan> {
    let mut roots = Vec::new();
    let mut infos = Vec::new();
    let mut scripts = Vec::new();
    let companions = companion_models(sp, Some(dp), models)?;
    let all: Vec<(String, bool)> = models
        .iter()
        .map(|m| (m.clone(), true))
        .chain(
            companions
                .iter()
                .filter(|c| !models.contains(c))
                .map(|c| (c.clone(), false)),
        )
        .collect();
    for (m, strict) in &all {
        let (m, strict) = (m.as_str(), *strict);
        for class in REGISTERED {
            need(
                dp.named(class, m)?.is_none(),
                "destination already carries the model",
            )?;
            match find_named(sp, class, m)? {
                Some(id) => {
                    roots.push(id);
                    infos.push(id);
                }
                None if strict => return Err(bad(&format!("source lacks {class} {m}"))),
                None => {}
            }
        }
        // A bullet is drawn however its source stage draws it; one the destination cannot
        // hang in the same way is registered without a scene node.
        let scene = if strict {
            Some(model_scene(sp, m)?)
        } else {
            model_scene(sp, m).ok().filter(|s| match s {
                ModelScene::Info(_) => dp.group(b"EM01").is_err(),
                ModelScene::Group(name) => dp.group(name).is_ok(),
            })
        };
        if let Some(ModelScene::Info(id)) = scene {
            need(
                dp.named("igSceneInfo", m)?.is_none(),
                "destination already carries the model's scene",
            )?;
            need(
                dp.group(b"EM01").is_err(),
                "a one-model scene cannot join a stage with an enemy group",
            )?;
            roots.push(id);
            infos.push(id);
        }
        for prefix in prefixes {
            let name = format!("{prefix}{m}.lua");
            if let Some(id) = find_named(sp, "beLuaData", &name)? {
                need(
                    dp.named("beLuaData", &name)?.is_none(),
                    "destination already carries the script",
                )?;
                roots.push(id);
                scripts.push((name.into_bytes(), id));
            }
        }
    }
    // Effects the source stage registers that this stage lacks.
    for (class, name, id) in effect_infos(sp)? {
        if !dp.has_effect(&class, &name)? {
            roots.push(id);
            infos.push(id);
        }
    }
    roots.extend(attached.iter().map(|&(top, _)| top));
    let (ids, owners) = closure(sp, &roots)?;
    check_blocks(sp, &ids, &owners)?;
    Ok(CopyPlan {
        infos,
        scripts,
        all,
        ids,
        owners,
    })
}

/// Append a closed subgraph with translated object, memory and class references.
pub(super) fn copy_graph(
    dp: &Pack,
    sp: &Pack,
    builder: &mut Builder,
    ids: &BTreeSet<u32>,
    owners: &HashMap<u32, usize>,
    report: &mut Report,
) -> Result<HashMap<u32, u32>> {
    let base = dp.ids.len() as u32;
    let map: HashMap<u32, u32> = ids
        .iter()
        .enumerate()
        .map(|(k, &s)| (s, base + k as u32))
        .collect();
    need(
        dp.ids.len() + ids.len() < u16::MAX as usize || builder.reuses_directories(),
        &format!(
            "grafted pack exceeds 16-bit directory index ({} objects, {} more needed, {} allowed)",
            dp.ids.len(),
            ids.len(),
            u16::MAX
        ),
    )?;
    let find_class = |p: &Pack, name: &str| p.classes.iter().position(|c| c.name == name);
    let dst_object_dir =
        find_class(dp, "igObjectDirEntry").ok_or_else(|| bad("no object directory"))?;
    let dst_memory_dir =
        find_class(dp, "igMemoryDirEntry").ok_or_else(|| bad("no memory directory"))?;
    for &s in ids {
        let entry = &sp.dirs[sp.mapping[s as usize] as usize];
        let issued = if let Some(idx) = sp.ids[s as usize] {
            let r = &sp.objects[idx];
            let class = class_index(dp, sp, r.class)?;
            let changes = ref_slots(sp, r)
                .map(|(n, _)| {
                    let v = r.val(sp.b, n)?;
                    Ok((n, if v == NULL { NULL } else { map[&v] }))
                })
                .collect::<Result<Vec<_>>>()?;
            let mut payload = r.patch(sp.b, &changes)?;
            put(&mut payload, 0, class as u32);
            let mut dir = entry.patch(sp.b, &[(11, class as u32)])?;
            put(&mut dir, 0, dst_object_dir as u32);
            report.objects += 1;
            report.object_bytes += payload.len();
            builder.add(dir, payload, false)?
        } else {
            let range = sp.memory[s as usize]
                .clone()
                .ok_or_else(|| bad("missing block"))?;
            let mut bytes = sp.b[range].to_vec();
            if let Some(&count) = owners.get(&s) {
                for i in 0..count {
                    let v = u32at(&bytes, 4 * i)?;
                    if v != NULL {
                        put(&mut bytes, 4 * i, map[&v]);
                    }
                }
            }
            let mut dir = sp.b[entry.range.clone()].to_vec();
            put(&mut dir, 0, dst_memory_dir as u32);
            report.blocks += 1;
            report.block_bytes += bytes.len();
            builder.add(dir, bytes, true)?
        };
        need(issued == map[&s], "new ID drifted from the plan")?;
    }

    Ok(map)
}

/// Re-read the result and prove that every copied record matches its source with only
/// IDs and class indexes changed, and that nothing original moved.
pub(super) fn verify(
    dp: &Pack,
    sp: &Pack,
    out: &[u8],
    map: &HashMap<u32, u32>,
    owners: &HashMap<u32, usize>,
    grows: &[(usize, usize)],
    rewrites: &BTreeMap<u32, Vec<u8>>,
) -> Result<()> {
    let v = Pack::parse(out)?;
    need(
        v.ids[..dp.ids.len()] == dp.ids[..]
            && dp.mapping.iter().enumerate().all(|(id, &entry)| {
                v.mapping[id] == entry || (dp.memory[id].is_some() && v.memory[id].is_some())
            }),
        "graft changed original IDs",
    )?;
    verify_pool_rewrites(dp, &v, grows, rewrites)?;
    for (&s, &d) in map {
        if let Some(si) = sp.ids[s as usize] {
            let di = v.ids[d as usize].ok_or_else(|| bad("copied object missing"))?;
            let (sr, dr) = (&sp.objects[si], &v.objects[di]);
            need(
                sp.classes[sr.class].name == v.classes[dr.class].name,
                "copied object changed class",
            )?;
            need(
                sr.fields.len() == dr.fields.len(),
                "copied object changed shape",
            )?;
            let slots = ref_slots(sp, sr).map(|(n, _)| n).collect::<BTreeSet<_>>();
            for (n, range) in &sr.fields {
                let destination_range = dr.field(*n)?;
                let (a, b) = (&sp.b[range.clone()], &v.b[destination_range.clone()]);
                if slots.contains(n) {
                    let x = u32at(sp.b, range.start)?;
                    let want = if x == NULL { NULL } else { map[&x] };
                    need(
                        u32at(v.b, destination_range.start)? == want,
                        "copied reference differs",
                    )?;
                } else {
                    need(a == b, "copied field differs")?;
                }
            }
        } else {
            let (sr, dr) = (
                sp.memory[s as usize].clone().unwrap(),
                v.memory[d as usize]
                    .clone()
                    .ok_or_else(|| bad("copied block missing"))?,
            );
            need(sr.len() == dr.len(), "copied block changed size")?;
            if let Some(&count) = owners.get(&s) {
                for i in 0..count {
                    let x = u32at(sp.b, sr.start + 4 * i)?;
                    let want = if x == NULL { NULL } else { map[&x] };
                    need(
                        u32at(v.b, dr.start + 4 * i)? == want,
                        "copied list element differs",
                    )?;
                }
                need(
                    sp.b[sr.start + 4 * count..sr.end] == v.b[dr.start + 4 * count..dr.end],
                    "copied list tail differs",
                )?;
            } else {
                need(sp.b[sr] == v.b[dr], "copied block differs")?;
            }
        }
    }
    Ok(())
}

/// Original memory stays byte-for-byte intact around additions to its blocks.
pub(super) fn verify_pool(dp: &Pack, v: &Pack, grows: &[(usize, usize)]) -> Result<()> {
    verify_pool_contents(&dp.b[dp.pool..], v, grows)
}

fn verify_pool_rewrites(
    dp: &Pack,
    v: &Pack,
    grows: &[(usize, usize)],
    rewrites: &BTreeMap<u32, Vec<u8>>,
) -> Result<()> {
    if rewrites.is_empty() {
        return verify_pool(dp, v, grows);
    }
    let mut expected = dp.b[dp.pool..].to_vec();
    for (&block, bytes) in rewrites {
        let before = dp.memory[block as usize].as_ref().unwrap();
        let after = v.memory[block as usize]
            .as_ref()
            .ok_or_else(|| bad("rewritten memory block missing"))?;
        need(bytes.len() >= before.len(), "rewritten memory shrank")?;
        need(&v.b[after.clone()] == bytes, "rewritten memory differs")?;
        expected[before.start - dp.pool..before.end - dp.pool]
            .copy_from_slice(&bytes[..before.len()]);
    }
    verify_pool_contents(&expected, v, grows)
}

fn verify_pool_contents(original: &[u8], v: &Pack, grows: &[(usize, usize)]) -> Result<()> {
    let (mut from, mut to, mut seen) = (0, v.pool, 0);
    for &(at, len) in grows {
        let run = at - seen;
        need(
            original[from..from + run] == v.b[to..to + run],
            "changed original memory pool",
        )?;
        from += run;
        to += run + len;
        seen = at;
    }
    need(
        original[from..] == v.b[to..to + original.len() - from],
        "changed original memory pool",
    )
}

/// The row of `donor` standing at the same position in its model family as `row` does in its own.
fn donor_row(row: u32, donor: &str) -> Result<u32> {
    let groups = crate::rowgroup::ROW_GROUPS;
    let from = groups
        .iter()
        .find(|g| g.rows.iter().any(|&r| r as u32 == row))
        .ok_or_else(|| bad(&format!("row {row} belongs to no model family")))?;
    let to = groups
        .iter()
        .find(|g| g.model == donor)
        .ok_or_else(|| bad(&format!("no model family {donor}")))?;
    let k = from.rows.iter().position(|&r| r as u32 == row).unwrap();
    Ok(to.rows[k.min(to.rows.len() - 1)] as u32)
}

/// Whether the pack already carries the model's registration records.
pub(super) fn resident(dst: &[u8], model: &str) -> Result<bool> {
    let dp = Pack::parse(dst)?;
    Ok(find_named(&dp, "beModelCtrlInfo", model)?.is_some())
}

/// Retarget generator enemy rows in place. Returns the new pack and how many changed.
pub(super) fn retarget(
    b: &[u8],
    rows: &BTreeMap<u32, u32>,
    spread: &[BTreeMap<u32, u32>],
    only: &[String],
    by_name: &BTreeMap<String, String>,
) -> Result<(Vec<u8>, usize)> {
    let p = Pack::parse(b)?;
    let mut edits = Vec::new();
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let r = &p.objects[*index];
        if p.classes[r.class].name != "beGeneraterData" {
            continue;
        }
        if !only.is_empty()
            && !name_of(&p, r).is_some_and(|n| only.iter().any(|o| o.as_bytes() == n))
        {
            continue;
        }
        let row = p.val(id as u32, 7)?;
        let named = name_of(&p, r).and_then(|n| by_name.get(&*String::from_utf8_lossy(n)));
        let new = match spread.first() {
            _ if named.is_some() => Some(donor_row(row, named.unwrap())?),
            Some(first) if first.contains_key(&row) => {
                // The name, not the pack, picks the model, so every tier agrees.
                let name = name_of(&p, r).unwrap_or_default();
                let hash = name.iter().fold(0xcbf29ce4u32, |h, &c| {
                    (h ^ c as u32).wrapping_mul(0x01000193)
                });
                spread[(hash >> 8) as usize % spread.len()]
                    .get(&row)
                    .copied()
            }
            _ => rows.get(&row).copied(),
        };
        if let Some(new) = new {
            edits.push(Edit {
                range: r.field(7)?.clone(),
                bytes: new.to_le_bytes().to_vec(),
            });
        }
    }
    edits.sort_by_key(|e| e.range.start);
    let n = edits.len();
    Ok((materialize(b, &edits)?, n))
}

impl Spec {
    /// The Abyss of Grief first-entry cutscene: only the intro Ogremon generator (`GENE-00`)
    /// spawns `donor` (copied from `source`). With `follow_script`, the cutscene's
    /// `e_ogre` constants are renamed to match; without it they are left as shipped.
    pub fn abyss_intro(source: usize, donor: &str, follow_script: bool) -> Self {
        let mut spec = Self::abyss_mummy();
        spec.source_entry = source;
        spec.models = vec![donor.to_string()];
        spec.rows.clear();
        let group = |m: &str| crate::rowgroup::ROW_GROUPS.iter().find(|g| g.model == m);
        let (from, to) = (group("e_ogre").unwrap(), group(donor).unwrap());
        for (k, &row) in from.rows.iter().enumerate() {
            spec.rows
                .insert(row as u32, to.rows[k.min(to.rows.len() - 1)] as u32);
        }
        spec.only_generators = vec!["GENE-00".into()];
        if follow_script {
            spec.lua_edits.push(LuaEdit {
                entry: 621,
                scripts: vec!["first.lua".into()],
                names: BTreeMap::from([("e_ogre".to_string(), donor.to_string())]),
            });
        }
        spec
    }
}

impl Spec {
    /// As [`Self::abyss_intro`], but whole families move: every Ogremon spawn becomes `intro`
    /// and every Goburimon spawn (the crown generators included) becomes `goburi`, both
    /// `(source entry, model)`. Tests an intro whose model is also spawned elsewhere.
    pub fn abyss_intro_families(
        intro: (usize, &str),
        goburi: (usize, &str),
        follow_script: bool,
    ) -> Self {
        let mut spec = Self::abyss_intro(intro.0, intro.1, follow_script);
        spec.only_generators.clear();
        let group = |m: &str| {
            crate::rowgroup::ROW_GROUPS
                .iter()
                .find(|g| g.model == m)
                .unwrap()
        };
        let (from, to) = (group("e_goburi"), group(goburi.1));
        for (k, &row) in from.rows.iter().enumerate() {
            spec.rows
                .insert(row as u32, to.rows[k.min(to.rows.len() - 1)] as u32);
        }
        if goburi != intro {
            if goburi.0 == intro.0 {
                spec.models.push(goburi.1.to_string());
            } else {
                spec.extra_sources
                    .push((goburi.0, vec![goburi.1.to_string()]));
            }
        }
        spec
    }
}

/// A stage whose first-entry cutscene waits on an enemy model by name in its Lua.
pub struct IntroStage {
    pub model_entry: usize,
    /// The three difficulty-tier generator packs.
    pub generators: &'static [usize],
    /// The `beLuaData` script holding the cutscene.
    pub script: &'static str,
    /// `(generator, model it spawns)` for each cutscene enemy.
    pub sites: &'static [(&'static str, &'static str)],
}

pub const INTRO_STAGES: &[IntroStage] = &[
    IntroStage {
        model_entry: 241,
        generators: &[245, 246, 247],
        script: "first.lua",
        sites: &[("INTRO-01", "e_minotaru")],
    },
    IntroStage {
        model_entry: 394,
        generators: &[398, 399, 400],
        script: "first.lua",
        sites: &[("INTRO-01", "e_rare")],
    },
    IntroStage {
        model_entry: 187,
        generators: &[191, 192, 193],
        script: "first.lua",
        sites: &[("GENE-01", "e_scum")],
    },
    IntroStage {
        model_entry: 257,
        generators: &[261, 262, 263],
        script: "matg05.lua",
        sites: &[("GENE-06", "e_pharaoh")],
    },
    IntroStage {
        model_entry: 614,
        generators: &[615, 616, 617],
        script: "first.lua",
        sites: &[("GENE-21", "e_goburi"), ("GENE-22", "e_nume")],
    },
    IntroStage {
        model_entry: 621,
        generators: &[622, 623, 624],
        script: "first.lua",
        sites: &[("GENE-00", "e_ogre")],
    },
    IntroStage {
        model_entry: 635,
        generators: &[636, 637, 638],
        script: "first.lua",
        sites: &[("GENE-018", "e_cockatri")],
    },
    IntroStage {
        model_entry: 642,
        generators: &[643, 644, 645],
        script: "first.lua",
        sites: &[("GENE-49", "e_mummy"), ("GENE-50", "e_tyrano")],
    },
    IntroStage {
        model_entry: 649,
        generators: &[652, 653, 654],
        script: "first.lua",
        sites: &[("GENE-00", "e_arachne")],
    },
    IntroStage {
        model_entry: 679,
        generators: &[680, 681, 682],
        script: "first.lua",
        sites: &[("GENE-89", "e_otama"), ("GENE-90", "e_geko")],
    },
    IntroStage {
        model_entry: 686,
        generators: &[689, 690, 691],
        script: "first.lua",
        sites: &[("GENE-165", "e_torta")],
    },
    IntroStage {
        model_entry: 709,
        generators: &[710, 711, 712],
        script: "first.lua",
        sites: &[("GENE-54", "e_infermo"), ("GENE-55", "e_megadra")],
    },
    IntroStage {
        model_entry: 723,
        generators: &[724, 725, 726],
        script: "first.lua",
        sites: &[("GENE-01", "e_impdra")],
    },
];

impl Spec {
    /// Make every cutscene enemy of `stage` a donor model. `picks` gives, for each original
    /// model, `(donor source entry, donor model)`. Donors the stage already carries are not
    /// copied. With `follow_script` the cutscene's name constants are renamed to match; with
    /// `warp` the Goblin Pass exit (entry 614) leads to that stage instead of the Abyss.
    pub fn intro_stage(
        stage: &IntroStage,
        picks: &[(&str, usize, &str)],
        follow_script: bool,
        warp: Option<&str>,
    ) -> Result<Self> {
        let mut spec = Self::abyss_mummy();
        spec.model_entry = stage.model_entry;
        spec.generator_entries = stage.generators.to_vec();
        spec.rows.clear();
        spec.models.clear();
        spec.extra_sources.clear();
        spec.source_entry = picks.first().map_or(0, |p| p.1);
        let mut names = BTreeMap::new();
        for &(orig, src, donor) in picks {
            if src == spec.source_entry {
                spec.models.push(donor.to_string());
            } else if let Some(g) = spec.extra_sources.iter_mut().find(|g| g.0 == src) {
                g.1.push(donor.to_string());
            } else {
                spec.extra_sources.push((src, vec![donor.to_string()]));
            }
            names.insert(orig.to_string(), donor.to_string());
        }
        for &(generator, orig) in stage.sites {
            let donor = names
                .get(orig)
                .ok_or_else(|| bad(&format!("no donor picked for {orig}")))?;
            spec.generator_models
                .insert(generator.to_string(), donor.clone());
        }
        if follow_script {
            spec.lua_edits.push(LuaEdit {
                entry: stage.model_entry,
                scripts: vec![stage.script.to_string()],
                names,
            });
        }
        if let Some(target) = warp {
            need(
                stage.model_entry != 614,
                "the Goblin Pass is entered from the gate",
            )?;
            spec.lua_edits.push(LuaEdit {
                entry: 614,
                scripts: vec!["matz02.lua".into()],
                names: BTreeMap::from([("stag0102".to_string(), target.to_string())]),
            });
        }
        Ok(spec)
    }
}

/// Per-pack generator edits chosen by name: enemy rows and spawn-sequence overrides.
#[derive(Default, Clone, Debug)]
pub(super) struct Retarget {
    pub rows: BTreeMap<String, u32>,
    pub blank_seq: BTreeSet<String>,
    pub sequences: BTreeMap<String, String>,
}

/// Field numbers of `beGeneraterData` and its per-player counts.
pub(super) const GENERATOR_ROW: u16 = 7;
pub(super) const GENERATOR_SEQ: u16 = 8;

pub(super) fn retarget_rows(b: &[u8], rt: &Retarget) -> Result<(Vec<u8>, usize)> {
    let p = Pack::parse(b)?;
    let mut edits = Vec::new();
    let mut n = 0;
    let mut added = 0;
    for index in p.ids.iter().flatten() {
        let r = &p.objects[*index];
        if p.classes[r.class].name != "beGeneraterData" {
            continue;
        }
        let Some(name) = name_of(&p, r).map(|n| String::from_utf8_lossy(n).into_owned()) else {
            continue;
        };
        if let Some(&row) = rt.rows.get(&name) {
            edits.push(Edit {
                range: r.field(GENERATOR_ROW)?.clone(),
                bytes: row.to_le_bytes().to_vec(),
            });
            n += 1;
        }
        if rt.blank_seq.contains(&name) {
            let f = r.field(GENERATOR_SEQ)?;
            need(f.len() > 4, "generator sequence field too short")?;
            edits.push(Edit {
                range: f.start + 4..f.end,
                bytes: vec![0; f.len() - 4],
            });
        }
        if let Some(sequence) = rt.sequences.get(&name) {
            need(!rt.blank_seq.contains(&name), "conflicting spawn sequences")?;
            let f = r.field(GENERATOR_SEQ)?;
            let size = a4(sequence.len() + 1).max(f.len() - 4);
            let mut bytes = (size as u32).to_le_bytes().to_vec();
            bytes.extend(sequence.as_bytes());
            bytes.resize(4 + size, 0);
            let growth = bytes.len() - f.len();
            edits.push(Edit { range: f, bytes });
            if growth > 0 {
                edits.push(Edit {
                    range: r.range.start + 4..r.range.start + 8,
                    bytes: ((r.range.len() + growth) as u32).to_le_bytes().to_vec(),
                });
                added += growth;
            }
        }
    }
    if added > 0 {
        edits.push(Edit {
            range: 16..20,
            bytes: (u32at(b, 16)? + added as u32).to_le_bytes().to_vec(),
        });
    }
    edits.sort_by_key(|e| e.range.start);
    let out = materialize(b, &edits)?;
    let after = Pack::parse(&out)?;
    need(
        p.mapping == after.mapping && b[p.pool..] == out[after.pool..],
        "generator retarget changed IDs or memory",
    )?;
    Ok((out, n))
}

/// What copying a model out of a stage pack takes, worked out without a destination.
pub(super) struct Profile {
    /// Definitions of the classes the copy uses; the destination gains any it lacks.
    pub classes: Vec<ClassDef>,
    /// Concurrent instances the engine keeps for the model.
    pub ram: u32,
    /// How the model hangs in the scene.
    pub scene: ModelScene,
}

impl Profile {
    /// The classes `dp` still has to define before it can take the copy.
    pub(super) fn missing(&self, dp: &Pack) -> Vec<ClassDef> {
        self.missing_in(&PackDestination::new(dp))
    }

    pub(super) fn missing_in(&self, dp: &dyn Destination) -> Vec<ClassDef> {
        self.classes
            .iter()
            .filter(|c| !dp.classes().iter().any(|d| d.name == c.class.name))
            .cloned()
            .collect()
    }

    /// Why `dp` cannot take the copy: a class laid out differently, or no scene group to hang it on.
    pub(super) fn misfit(&self, dp: &Pack) -> Option<String> {
        self.misfit_in(&PackDestination::new(dp))
    }

    pub(super) fn misfit_in(&self, dp: &dyn Destination) -> Option<String> {
        if let Some(c) = self.classes.iter().find(|c| {
            dp.classes()
                .iter()
                .any(|d| d.name == c.class.name && d.fields != c.class.fields)
        }) {
            return Some(format!("class {} is laid out differently", c.class.name));
        }
        let grouped = dp.group(b"EM01").is_ok();
        match &self.scene {
            ModelScene::Group(name) if dp.group(name).is_err() => Some("no enemy group".into()),
            ModelScene::Info(_) if grouped => Some("stage has an enemy group".into()),
            _ => None,
        }
    }
}

/// Run every source-side check [`graft`] makes, and refuse hit scripts that touch the
/// kill-count quest counters (those stages count kills in the model's own script).
pub(super) fn profile(sp: &Pack, model: &str) -> Result<Profile> {
    profile_with_scripts(sp, model, &MODEL_SCRIPTS)
}

pub(super) fn profile_with_scripts(sp: &Pack, model: &str, prefixes: &[&str]) -> Result<Profile> {
    let _timing = crate::profiling::measure("graft.profile");
    let RenderSource {
        bytes: rendered,
        attached,
    } = render_source(sp, None, &[model.to_string()], &mut Report::default())?;
    let source = Pack::parse(&rendered)?;
    let (mut roots, ram) = model_roots_with_scripts(&source, model, prefixes)?;
    roots.extend(attached.iter().map(|&(top, _)| top));
    profile_of(&source, model, roots, ram)
}

/// The records [`graft`] copies for a donor model in `sp`, and the model's RAM budget.
#[cfg(all(test, feature = "devtools"))]
pub(super) fn model_roots(sp: &Pack, model: &str) -> Result<(Vec<u32>, u32)> {
    model_roots_with_scripts(sp, model, &MODEL_SCRIPTS)
}

fn model_roots_with_scripts(sp: &Pack, model: &str, prefixes: &[&str]) -> Result<(Vec<u32>, u32)> {
    let mut roots = Vec::new();
    let mut ram = 0;
    for class in REGISTERED {
        let id = find_named(sp, class, model)?
            .ok_or_else(|| bad(&format!("source lacks {class} {model}")))?;
        if class == "beModelCtrlInfo" {
            ram = sp.val(sp.val(id, 6)?, 2)?;
        }
        roots.push(id);
    }
    for prefix in prefixes {
        let name = format!("{prefix}{model}.lua");
        if let Some(id) = find_named(sp, "beLuaData", &name)? {
            let object = sp.val(id, 3)?;
            let (size, mem) = (sp.val(object, 2)? as usize, sp.val(object, 3)?);
            let range = sp
                .memory
                .get(mem as usize)
                .and_then(|r| r.as_ref())
                .ok_or_else(|| bad("missing Lua memory"))?;
            let code = &sp.b[range.start..range.start + size.min(range.len())];
            for banned in [&b"COUNTERset\0"[..], b"NUMBERCTRL\0"] {
                need(
                    memchr::memmem::find(code, banned).is_none(),
                    "hit script drives a quest counter",
                )?;
            }
            roots.push(id);
        }
    }
    for (_, _, id) in effect_infos(sp)? {
        roots.push(id);
    }
    let bullets = companion_models(sp, None, &[model.to_string()])?;
    for m in &bullets {
        for class in REGISTERED {
            if let Some(id) = find_named(sp, class, m)? {
                roots.push(id);
            }
        }
    }
    let mut units = BTreeSet::new();
    let scene_index = SceneIndex::new(sp);
    for (switch, _, _) in effect_switches(
        sp,
        None,
        &[model.to_string()],
        &bullets.iter().collect::<Vec<_>>(),
    )? {
        if let Some(at) = attach_point(sp, switch, &scene_index)? {
            units.insert(at.unit);
        }
    }
    roots.extend(units);
    for bullet in companion_models(sp, None, &[model.to_string()])? {
        for prefix in prefixes {
            if let Some(id) = find_named(sp, "beLuaData", &format!("{prefix}{bullet}.lua"))? {
                roots.push(id);
            }
        }
    }
    Ok((roots, ram))
}

fn profile_of(sp: &Pack, model: &str, mut roots: Vec<u32>, ram: u32) -> Result<Profile> {
    let scene = model_scene(sp, model)?;
    if let ModelScene::Info(id) = scene {
        roots.push(id);
    }
    let (ids, owners) = closure(sp, &roots)?;
    check_blocks(sp, &ids, &owners)?;
    let defs = class_defs(sp.b)?;
    let mut used: BTreeSet<usize> = ids
        .iter()
        .filter_map(|&id| sp.ids[id as usize].map(|i| sp.objects[i].class))
        .collect();
    // Parents come first (they have lower indexes), so close the set over them.
    let mut pending: Vec<usize> = used.iter().copied().collect();
    while let Some(c) = pending.pop() {
        if let Some(parent) = &defs[c].parent {
            let p = defs
                .iter()
                .position(|d| &d.class.name == parent)
                .ok_or_else(|| bad("class parent missing"))?;
            if used.insert(p) {
                pending.push(p);
            }
        }
    }
    let classes = used.into_iter().map(|c| defs[c].clone()).collect();
    Ok(Profile {
        classes,
        ram,
        scene,
    })
}

/// Whether the pack has an `igGroup` with this name.
pub(super) fn has_group(p: &Pack, name: &[u8]) -> bool {
    group_children(p, name).is_ok()
}

/// The scene nodes [`graft`] would hang for a model's bullets and effects in an identical stage.
#[cfg(all(test, feature = "devtools"))]
pub(super) fn scene_units(sp: &Pack, model: &str) -> Result<Vec<u32>> {
    let scene_index = SceneIndex::new(sp);
    let bullets = companion_models(sp, None, &[model.to_string()])?;
    let mut units = Vec::new();
    for (switch, _, _) in effect_switches(
        sp,
        None,
        &[model.to_string()],
        &bullets.iter().collect::<Vec<_>>(),
    )? {
        if let Some(at) = attach_point(sp, switch, &scene_index)? {
            units.push(at.unit);
        }
    }
    Ok(units)
}

#[cfg(test)]
#[path = "particle_tests.rs"]
mod particle_tests;

#[cfg(test)]
mod scene_index_tests {
    use super::*;

    #[test]
    fn importing_after_list_compaction_fills_spare_capacity_before_growing() {
        let original = crate::formats::igb::fixtures::object_graph();
        let p = Pack::parse(&original).unwrap();
        let mut builder = Builder::new(&p);
        builder.modify(1, &[(2, 1), (3, 1)]).unwrap();
        let compacted = materialize(&original, &builder.finish(0).unwrap()).unwrap();
        let p = Pack::parse(&compacted).unwrap();
        assert_eq!(p.array(1).unwrap(), [2]);
        assert_eq!(p.memory[0].as_ref().unwrap().len(), 8);
        for entries in [vec![2], vec![2, 2], vec![2, 2, 2, 2]] {
            let mut builder = Builder::new(&p);
            write_list(&p, &mut builder, 1, &entries).unwrap();
            let grows = builder.grown();
            let rewrites = builder.rewrites.clone();
            let out = materialize(&compacted, &builder.finish(0).unwrap()).unwrap();
            let after = Pack::parse(&out).unwrap();
            assert_eq!(after.array(1).unwrap(), entries);
            assert_eq!(
                after.val(1, 4).unwrap(),
                0,
                "backing block must remain owned"
            );
            assert_eq!(after.val(1, 3).unwrap(), entries.len() as u32);
            assert_eq!(
                p.b[p.memory[3].clone().unwrap()],
                after.b[after.memory[3].clone().unwrap()],
                "another block sharing the descriptor changed"
            );
            verify_pool_rewrites(&p, &after, &grows, &rewrites).unwrap();
        }
    }

    #[test]
    fn shared_directories_keep_graft_object_references_32_bit() {
        let source_bytes = crate::formats::igb::fixtures::object_graph();
        let source = Pack::parse(&source_bytes).unwrap();
        let mut builder = Builder::new(&source).reuse_directories();
        for _ in 0..u16::MAX as usize {
            builder.mem(words(&[NULL]), 0).unwrap();
        }
        let bytes = materialize(&source_bytes, &builder.finish(0).unwrap()).unwrap();
        let destination = Pack::parse(&bytes).unwrap();
        assert!(destination.ids.len() > u16::MAX as usize);
        assert!(destination.dirs.len() < 10);
        let (ids, owners) = closure(&source, &[1]).unwrap();
        let mut builder = Builder::new(&destination).reuse_directories();
        let map = copy_graph(
            &destination,
            &source,
            &mut builder,
            &ids,
            &owners,
            &mut Report::default(),
        )
        .unwrap();
        let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
        let result = Pack::parse(&out).unwrap();
        assert!(map[&2] > u16::MAX as u32);
        assert_eq!(result.array(map[&1]).unwrap(), [map[&2], NULL]);
        assert_eq!(result.array(1).unwrap(), [2, NULL]);
    }

    #[test]
    fn duplicate_children_owner_fields_and_unregistered_owners_keep_the_old_semantics() {
        let mut bytes = crate::formats::igb::fixtures::object_graph();
        let pool = Pack::parse(&bytes).unwrap().pool;
        put(&mut bytes, pool + 4, 2); // The same child twice in the node list.
        let mut p = Pack::parse(&bytes).unwrap();
        p.classes[3].name = "igNodeList".into();
        p.classes[4].fields = vec![(0, 2, 4), (0, 3, 4)];
        let root = p.table + 8 + 2 * p.ids.len(); // Contains node-list ID 1.
        p.objects[1].fields = BTreeMap::from([(2, root..root + 4), (3, root..root + 4)]).into();
        let orphan = p.objects[1].clone();
        p.objects.push(orphan);
        p.ids[3] = Some(2); // An owner not registered in the root list.
        p.memory[3] = None;
        let index = SceneIndex::new(&p);
        assert_eq!(index.holders(2), [(2, 1), (2, 1), (3, 1), (3, 1)]);
        for child in 0..p.ids.len() as u32 {
            assert_eq!(index.holders(child), node_holders(&p, child));
        }
    }
}
