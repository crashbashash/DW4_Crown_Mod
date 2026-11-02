//! Assemble a portable donor from an enemy's separately loaded body and its
//! home stage's attack/script resources. The native encounter stays external.
use super::*;

pub(super) const MODEL: &str = "e_arachne";
pub(super) const STAGE: usize = 649;
pub(super) const ASSET: usize = 650;
pub(super) const AUDIO: usize = 648;
pub(super) const GROUP_TEMPLATE: usize = 187;
pub(super) const BULLET: &str = "x_009eblst01a";

/// The standalone file contains the body, animation, shadow and land-collision
/// records. Its stage supplies the hit script, web projectile and attack effects.
/// Joining their graphs in memory gives the normal importer a complete source;
/// no destination needs the home stage's separate-file load callback.
pub(super) fn arukenimon_source(
    stage: &[u8],
    asset: &[u8],
    group_template: &[u8],
) -> Result<Vec<u8>> {
    let native = Pack::parse(stage)?;
    need(
        graft::find_named(&native, "beModelCtrlInfo", MODEL)?.is_none(),
        "Arukenimon body is already embedded in its source stage",
    )?;
    let grouped = graft::add_enemy_group(asset, group_template, MODEL)?;
    let body = Pack::parse(&grouped)?;
    let profile = graft::profile(&body, MODEL)?;
    need(
        profile.misfit(&native).is_none(),
        "Arukenimon body is incompatible with its source stage",
    )?;
    let missing = profile.missing(&native);
    let base = if missing.is_empty() {
        stage.to_vec()
    } else {
        add_classes(stage, &missing)?
    };
    let (edits, report) = graft::graft(&base, &grouped, &[MODEL.to_string()])?;
    need(
        report.unattached.is_empty() && report.approximate.is_empty(),
        "Arukenimon source has unregistered rendering resources",
    )?;
    let out = materialize(&base, &edits)?;
    let combined = Pack::parse(&out)?;
    graft::profile(&combined, MODEL)?;
    need(
        graft::find_named(&combined, "beLuaData", "hit_e_arachne.lua")?.is_some()
            && graft::find_named(&combined, "beModelCtrlInfo", BULLET)?.is_some(),
        "Arukenimon source lacks its hit script or web projectile",
    )?;
    Ok(out)
}
