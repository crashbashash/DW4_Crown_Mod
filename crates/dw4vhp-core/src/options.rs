//! Feature selections shared by desktop requests and library callers.
//!
//! A selection starts empty. This differs deliberately from the historical
//! `PatchPlan::default()` balance preset; legacy image defaults are handled by
//! image metadata readers rather than by the request contract.
use crate::plan::PatchPlan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchSelection {
    pub crown_mod: bool,
    pub four_player_spawns: bool,
    pub crown_drops: bool,
    pub random_crown_colours: bool,
    pub random_enemies: bool,
    pub enemy_seed: Option<u32>,
}

impl PatchSelection {
    pub fn has_features(&self) -> bool {
        self.crown_mod
            || self.four_player_spawns
            || self.random_crown_colours
            || self.random_enemies
    }

    /// Build the same balance plan as the desktop selection, rejecting empty
    /// requests and dependent features that cannot be applied independently.
    pub fn into_plan(self) -> Result<PatchPlan, &'static str> {
        if self.crown_drops && !self.crown_mod {
            return Err("Crown Drops requires Crown Mod.");
        }
        if !self.has_features() {
            return Err("Select at least one patch option.");
        }
        Ok(PatchPlan {
            crown_mod: self.crown_mod,
            four_player_spawns: self.four_player_spawns,
            crown_drops: self.crown_drops,
            random_crown_colours: self.random_crown_colours,
            random_enemies: self.random_enemies,
            enemy_seed: self.enemy_seed.filter(|_| self.random_enemies),
            ..PatchPlan::default()
        })
    }

    /// Analysis estimates the same three resource options as before. Enemy
    /// planning and random cosmetic seeds are resolved by the patch worker.
    pub fn analysis_plan(self) -> PatchPlan {
        PatchPlan {
            crown_mod: self.crown_mod,
            four_player_spawns: self.four_player_spawns,
            crown_drops: self.crown_drops,
            ..PatchPlan::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_defaults_are_empty_and_use_camel_case() {
        let selection: PatchSelection = serde_json::from_value(json!({})).unwrap();
        assert_eq!(selection, PatchSelection::default());
        assert!(!selection.has_features());
        assert!(selection.into_plan().is_err());
        assert_eq!(
            serde_json::to_value(selection).unwrap(),
            json!({
                "crownMod": false, "fourPlayerSpawns": false, "crownDrops": false,
                "randomCrownColours": false, "randomEnemies": false, "enemySeed": null
            })
        );
    }

    #[test]
    fn validation_and_seed_handling_preserve_the_desktop_contract() {
        assert!(PatchSelection {
            crown_drops: true,
            ..Default::default()
        }
        .into_plan()
        .is_err());
        let random = PatchSelection {
            random_enemies: true,
            enemy_seed: Some(42),
            ..Default::default()
        }
        .into_plan()
        .unwrap();
        assert!(!random.crown_mod && !random.crown_drops);
        assert_eq!(random.enemy_seed, Some(42));
        let colours = PatchSelection {
            random_crown_colours: true,
            enemy_seed: Some(42),
            ..Default::default()
        }
        .into_plan()
        .unwrap();
        assert!(!colours.crown_mod);
        assert_eq!(colours.enemy_seed, None);
        let crown = PatchSelection {
            crown_mod: true,
            crown_drops: true,
            ..Default::default()
        }
        .into_plan()
        .unwrap();
        assert!(crown.crown_mod && crown.crown_drops);
    }

    #[test]
    fn analysis_keeps_optional_random_planning_out_of_inspection() {
        let plan = PatchSelection {
            random_enemies: true,
            random_crown_colours: true,
            four_player_spawns: true,
            ..Default::default()
        }
        .analysis_plan();
        assert!(!plan.crown_mod && plan.four_player_spawns);
        assert!(!plan.random_enemies && !plan.random_crown_colours);
    }

    #[test]
    fn malformed_seed_and_unknown_options_fail_at_the_boundary() {
        for input in [
            json!({"enemySeed": -1}),
            json!({"enemySeed": 4294967296u64}),
            json!({"crownMod": "true"}),
            json!({"typo": true}),
        ] {
            assert!(serde_json::from_value::<PatchSelection>(input).is_err());
        }
    }
}
