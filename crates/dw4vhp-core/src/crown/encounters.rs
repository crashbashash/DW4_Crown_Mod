//! Auditable enemy eligibility and fixed encounter protections.
//! Keep order and numeric values stable unless intentionally changing seed rules.

/// Ordinary enemies: the models that may be copied into other stages.
pub(crate) const REGULAR: &[&str] = &[
    "e_goburi",
    "e_nume",
    "e_scum",
    "e_ogre",
    "e_mecha1",
    "e_cockatri",
    "e_mummy",
    "e_mecha2",
    "e_tyrano",
    "e_minotaru",
    "e_otama",
    "e_geko",
    "e_maridevi",
    "e_mecha3",
    "e_infermo",
    "e_megadra",
    "e_rare",
    "e_impdra",
    "e_mecha4",
    "e_mecha5",
    "e_pharaoh",
    "e_arachne",
    "e_torta",
];
/// Enemies retired from stages without being offered as imports. None currently.
pub(crate) const REPLACEABLE: &[&str] = &[];
/// Custom encounter and tutorial packs never supply models to other stages.
pub(crate) const FORBIDDEN_SOURCES: &[usize] = &[449, 457, 464, 603, 607, 786];
/// The developer world (603) and the final bosses: left as shipped.
pub(crate) const SKIPPED: &[usize] = &[603, 730, 737, 744, 751, 758, 765, 786];
/// A fixed generator protection, with the stage identity and rationale kept
/// alongside the policy that the randomizer actually consumes.
#[derive(Debug)]
pub struct EncounterProtection {
    pub pack_entry: usize,
    pub name: &'static str,
    pub stage: &'static str,
    pub generators: &'static [&'static str],
    pub reason: &'static str,
}

pub(crate) const PROTECTIONS: &[EncounterProtection] = &[
    EncounterProtection { pack_entry: 187, name: "Humid Cave B2", stage: "dng101b2", generators: &["GENE-08"], reason: "Preserve the native Blossomon boss. Sukamon's introduction retargets its wait and freeze/unfreeze commands to the replacement, allowing the intro and ordinary patrols to randomize." },
    EncounterProtection { pack_entry: 210, name: "Cliff Dungeon B3", stage: "dng102b3", generators: &["GENE-12"], reason: "Preserve the native Mammothmon encounter generator." },
    EncounterProtection { pack_entry: 241, name: "Dungeon dng204b2", stage: "dng204b2", generators: &["GENE-13"], reason: "Preserve the native SkullGreymon boss. The Minotaurmon tutorial introduction retargets its wait and control commands." },
    EncounterProtection { pack_entry: 265, name: "Dungeon dng205b3", stage: "dng205b3", generators: &["GENE-15"], reason: "Preserve the native scripted encounter. The boss intro erases the floor’s patrol families; those randomise with the erase following their replacements." },
    EncounterProtection { pack_entry: 296, name: "Dungeon dng307b2", stage: "dng307b2", generators: &["GENE-11"], reason: "Preserve the native encounter generator." },
    EncounterProtection { pack_entry: 319, name: "Dungeon dng308b3", stage: "dng308b3", generators: &["GENE-19"], reason: "MR04 is the native boss on this floor; preserve its scripted encounter." },
    EncounterProtection { pack_entry: 349, name: "Dungeon dng309b4", stage: "dng309b4", generators: &["GENE-24"], reason: "Preserve the native scripted encounter generator." },
    EncounterProtection { pack_entry: 394, name: "Dungeon dng411b3", stage: "dng411b3", generators: &["GENE-23"], reason: "Preserve the native MR05 boss. The Raremon tutorial introduction retargets its wait and control commands." },
    EncounterProtection { pack_entry: 425, name: "Dungeon dng412b4", stage: "dng412b4", generators: &["GENE-21", "GENE-20"], reason: "Preserve the native scripted encounter and associated generator." },
    EncounterProtection { pack_entry: 441, name: "Quest 1, stage 2", stage: "dngq1b2e", generators: &["GENE-06"], reason: "Preserve the native quest encounter generator." },
    EncounterProtection { pack_entry: 510, name: "Quest 4, stage 2 (Normal)", stage: "dngq4b2e", generators: &["GENE-11"], reason: "Preserve the native quest encounter generator." },
    EncounterProtection { pack_entry: 518, name: "Quest 4, stage 2 (Hard)", stage: "dngq4b2h", generators: &["GENE-11"], reason: "Preserve the native quest encounter generator." },
    EncounterProtection { pack_entry: 526, name: "Quest 4, stage 2 (Very Hard)", stage: "dngq4b2v", generators: &["GENE-11"], reason: "Preserve the native quest encounter generator." },
    EncounterProtection { pack_entry: 556, name: "Quest 7, stage 2", stage: "dngq7b2e", generators: &["GENE-22", "GENE-16", "GENE-17", "GENE-18", "GENE-19", "GENE-20", "GENE-21"], reason: "Preserve the native encounter and its scripted supporting generators." },
    EncounterProtection { pack_entry: 579, name: "Quest 8, stage 3 (Normal)", stage: "dngq8b3e", generators: &["GENE-22", "GENE-20", "GENE-21"], reason: "Preserve the native quest encounter and supporting generators." },
    EncounterProtection { pack_entry: 587, name: "Quest 8, stage 3 (Hard)", stage: "dngq8b3h", generators: &["GENE-22", "GENE-23"], reason: "Preserve the native quest encounter and supporting generators." },
    EncounterProtection { pack_entry: 595, name: "Quest 8, stage 3 (Very Hard)", stage: "dngq8b3v", generators: &["GENE-22", "GENE-23", "GENE-24"], reason: "Preserve the native quest encounter and supporting generators." },
    EncounterProtection { pack_entry: 621, name: "Abyss of Grief", stage: "stag0102", generators: &["GENE-38"], reason: "Preserve the generator addressed by the native stage scripts." },
    EncounterProtection { pack_entry: 635, name: "Doom Dome", stage: "stag0104", generators: &["GENE-011", "GENE-013", "GENE-016"], reason: "Preserve the two native scripted generators." },
    EncounterProtection { pack_entry: 649, name: "Grief Wasteland", stage: "stag0202", generators: &["GENE-00", "GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"], reason: "Preserve the separately loaded native Arukenimon introduction and scripted boss/add groups. Imported Arukenimon donors carry an embedded body instead." },
    EncounterProtection { pack_entry: 686, name: "Mangrove Forest", stage: "stag0302", generators: &["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"], reason: "Preserve ShogunGekomon's boss variants. The two preceding waves and Tortamon's introduction randomize with matching model-name waits and cleanup; generator callbacks retain wave progression." },
 ];

/// Fixed protections are supplemented by authored entrances and model names
/// addressed by native stage scripts; see docs/randomizer-rules.md.
pub fn protected_encounters() -> &'static [EncounterProtection] {
    PROTECTIONS
}

/// Native model names eligible for import, in the stable RNG selection order.
pub fn importable_models() -> &'static [&'static str] {
    REGULAR
}

/// Kill-count quest stages: their clear counts kills in the hit scripts of these models and the
/// enemy supply is fixed, so enemies only trade places among them and nothing is copied in.
pub(crate) const RESTRICTED: &[(usize, &[&str])] = &[
    // Timed quest: each native hit script updates the per-player kill counter.
    (449, &["e_mummy", "e_mecha1", "e_minotaru", "e_pharaoh"]),
    (457, &["e_mummy", "e_mecha2"]),
    (464, &["e_mummy", "e_mecha2"]),
];

/// Riverbank and train-side replacements need a projectile attack: melee enemies
/// cannot reach players across the gap. Entries require a registered attack spawn,
/// independent projectile motion and a damaging hitbox in retail data. Native
/// scripted entrances remain protected separately.
pub(crate) const RIVER_RANGED: &[&str] = &[
    "e_nume",
    "e_scum",
    "e_minotaru",
    "e_mecha2",
    "e_otama",
    "e_mecha3",
    "e_mecha4",
    "e_maridevi",
    "e_megadra",
    "e_infermo",
    "e_rare",
    "e_mecha5",
    "e_impdra",
];

/// Projectile-capable replacement families for the river and train, in policy order.
pub fn river_ranged_models() -> &'static [&'static str] {
    RIVER_RANGED
}

pub(crate) fn ranged_stage(entry: usize) -> bool {
    matches!(entry, 628 | 658)
}

/// Mangrove keeps its separate Tortamon preload, so an embedded import there would
/// register the same model name twice.
const SEPARATE_TORTAMON: usize = 686;

pub(crate) fn replacement_allowed(entry: usize, model: &str) -> bool {
    if entry == SEPARATE_TORTAMON && model == "e_torta" {
        return false;
    }
    !ranged_stage(entry) || RIVER_RANGED.contains(&model)
}

/// Models that are a stage's boss though they are ordinary enemies elsewhere.
pub(crate) const BOSS_REGULAR: &[(usize, &str)] = &[(319, "e_mecha4"), (394, "e_mecha5")];

/// Reviewed stage commands which only erase/freeze these families. River calls
/// cover every selected bank family; other groups retain one exclusive replacement.
pub(crate) struct FamilyRemap {
    pub entry: usize,
    pub models: &'static [&'static str],
    pub scripts: &'static [&'static str],
    /// The scripts only erase the families when the boss intro starts, so
    /// Pharaohmon may summon them too (summons are erased with the patrols).
    pub shared_summons: bool,
}

pub(crate) const FAMILY_REMAPS: &[FamilyRemap] = &[
    FamilyRemap {
        entry: 628,
        models: &["e_nume"],
        scripts: &[
            "matb03.lua",
            "matb06.lua",
            "matb08.lua",
            "matb09.lua",
            "matb12.lua",
            "matf00.lua",
        ],
        shared_summons: false,
    },
    // Ancient Ruins B3: the boss intro erases the floor's four patrol families.
    FamilyRemap {
        entry: 265,
        models: &["e_pharaoh", "e_mummy", "e_mecha1", "e_mecha2"],
        scripts: &["matg15.lua"],
        shared_summons: true,
    },
    FamilyRemap {
        entry: 649,
        models: &["e_mummy", "e_tyrano"],
        scripts: &["mata00.lua", "mata01.lua"],
        shared_summons: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn off_platform_encounters_allow_projectiles_and_preserve_other_stage_choices() {
        for entry in [628, 658] {
            for model in REGULAR.iter().chain(REPLACEABLE) {
                assert_eq!(
                    replacement_allowed(entry, model),
                    RIVER_RANGED.contains(model)
                );
            }
            assert!(!replacement_allowed(entry, "e_mecha1"));
            assert!(!replacement_allowed(entry, "e_pharaoh"));
        }
        for entry in [614, 686] {
            assert!(REGULAR
                .iter()
                .filter(|model| (entry, **model) != (SEPARATE_TORTAMON, "e_torta"))
                .all(|model| replacement_allowed(entry, model)));
        }
        assert!(replacement_allowed(614, "e_torta"));
        assert!(!replacement_allowed(SEPARATE_TORTAMON, "e_torta"));
    }

    #[test]
    fn protections_are_named_unique_and_explain_their_scope() {
        let mut packs = BTreeSet::new();
        for rule in protected_encounters() {
            assert!(packs.insert(rule.pack_entry));
            assert!(!rule.name.is_empty() && !rule.stage.is_empty() && !rule.reason.is_empty());
            assert!(!rule.generators.is_empty());
            assert_eq!(
                rule.generators.len(),
                rule.generators.iter().collect::<BTreeSet<_>>().len()
            );
        }
        let humid = protected_encounters()
            .iter()
            .find(|r| r.pack_entry == 187)
            .unwrap();
        assert_eq!(humid.stage, "dng101b2");
        assert_eq!(humid.generators, ["GENE-08"]);
        assert!(humid.reason.contains("Blossomon") && humid.reason.contains("retargets"));
    }

    #[test]
    fn regular_mr04_is_importable_while_its_separate_boss_model_is_not() {
        assert!(importable_models().contains(&"e_mecha4"));
        assert!(!importable_models().contains(&"e_mecha4bs"));
        assert_eq!(
            importable_models().len(),
            importable_models().iter().collect::<BTreeSet<_>>().len()
        );
    }
}
