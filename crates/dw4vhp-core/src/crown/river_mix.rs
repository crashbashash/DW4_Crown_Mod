//! Keep river bank cleanup and bomb freezes scoped to the selected ranged families.
use super::river::lua::{call, signed, Chunk};
use super::*;
use std::collections::BTreeSet;

pub(super) const MAX_FAMILIES: usize = 3;

fn fan_out(raw: &[u8], freeze: bool, families: &BTreeSet<String>) -> Result<Vec<u8>> {
    need(
        (2..=MAX_FAMILIES).contains(&families.len())
            && families
                .iter()
                .all(|m| encounters::RIVER_RANGED.contains(&m.as_str())),
        "unexpected mixed river families",
    )?;
    let mut c = Chunk::read(raw)?;
    let (scope, command) = if freeze {
        ("MODELCTRL", "ONFREEZE")
    } else {
        ("ENEMY", "SQ_ERAS")
    };
    let mut after = BTreeMap::new();
    let mut replace = BTreeMap::new();
    let mut count = 0;
    for pc in 0..c.code.len().saturating_sub(6) {
        if !c.is(c.code[pc + 3], 7, "e_nume") {
            continue;
        }
        need(
            c.is(c.code[pc], 12, "be")
                && c.is(c.code[pc + 1], 16, "sd")
                && c.is(c.code[pc + 2], 7, scope)
                && c.code[pc + 4] == signed(6, -1)
                && c.is(c.code[pc + 5], 7, command)
                && c.code[pc + 6] == call(0, 0),
            "unexpected river family command",
        )?;
        let mut members = families.iter();
        replace.insert(pc + 3, c.literal(members.next().unwrap()));
        let mut extra = Vec::new();
        for member in members {
            extra.extend(c.method("sd"));
            extra.extend([
                c.literal(scope),
                c.literal(member),
                signed(6, -1),
                c.literal(command),
                call(0, 0),
            ]);
        }
        after.insert(pc + 6, extra);
        count += 1;
    }
    need(
        count == 1,
        "river script must have exactly one family command",
    )?;
    need(
        c.code.iter().filter(|&&ins| c.is(ins, 7, "e_nume")).count() == 1,
        "unexpected additional river model reference",
    )?;
    let code = c.rewrite(&BTreeMap::new(), &after, &replace)?;
    c.write(&code)
}

pub(super) fn remap_scripts(
    b: &[u8],
    scripts: &[&str],
    families: &BTreeSet<String>,
) -> Result<Vec<Edit>> {
    need(!families.is_empty(), "river has no bank family")?;
    let scripts = scripts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if families.len() == 1 {
        return lua_names::rename_in_pack(
            b,
            &scripts,
            &BTreeMap::from([("e_nume".into(), families.first().unwrap().clone())]),
        );
    }
    lua_names::rewrite_in_pack(b, &scripts, |script, raw| {
        fan_out(raw, script == "matf00.lua", families)
    })
}

#[cfg(test)]
mod tests {
    use super::super::river::lua::{unsigned, BIAS};
    use super::*;

    fn chunk(freeze: bool) -> Vec<u8> {
        fn string(out: &mut Vec<u8>, s: &str) {
            out.extend((s.len() as u32 + 1).to_le_bytes());
            out.extend(s.as_bytes());
            out.push(0);
        }
        let mut b = b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08".to_vec();
        b.extend(314159265.3589793_f64.to_le_bytes());
        string(&mut b, "@river.lua");
        b.extend([0; 9]);
        b.extend(6u32.to_le_bytes());
        b.extend([0; 8]);
        b.extend(5u32.to_le_bytes());
        for s in [
            "be",
            "sd",
            if freeze { "MODELCTRL" } else { "ENEMY" },
            "e_nume",
            if freeze { "ONFREEZE" } else { "SQ_ERAS" },
        ] {
            string(&mut b, s);
        }
        b.extend([0; 8]); // no numbers or nested functions
        let code = [
            signed(42, 7),
            unsigned(12, 0),
            unsigned(16, 1),
            unsigned(7, 2),
            unsigned(7, 3),
            signed(6, -1),
            unsigned(7, 4),
            call(0, 0),
            0,
        ];
        b.extend((code.len() as u32).to_le_bytes());
        for ins in code {
            b.extend(ins.to_le_bytes());
        }
        b
    }

    #[test]
    fn mixed_commands_cover_each_family_and_relocate_skipping_branches() {
        for freeze in [false, true] {
            for n in [2, 3] {
                let families = ["e_scum", "e_mecha5", "e_rare"]
                    .into_iter()
                    .take(n)
                    .map(str::to_string)
                    .collect();
                let bytes = fan_out(&chunk(freeze), freeze, &families).unwrap();
                let c = Chunk::read(&bytes).unwrap();
                assert_eq!(c.code.len(), 9 + 7 * (n - 1));
                // Incoming skip still lands on END, beyond every inserted call.
                assert_eq!(1 + (c.code[0] >> 6) as i64 - BIAS, c.code.len() as i64 - 1);
                assert_eq!(c.code.iter().filter(|&&v| v == call(0, 0)).count(), n);
                let named: BTreeSet<_> = c
                    .code
                    .iter()
                    .filter(|&&v| v & 63 == 7)
                    .filter_map(|&v| c.text((v >> 6) as usize).ok())
                    .filter(|s| s.starts_with("e_"))
                    .map(str::to_string)
                    .collect();
                assert_eq!(named, families);
            }
        }
    }

    #[test]
    fn unexpected_scope_or_command_is_refused() {
        let families = BTreeSet::from(["e_scum".into(), "e_mecha5".into()]);
        assert!(fan_out(&chunk(true), false, &families).is_err());
        assert!(fan_out(&chunk(false), true, &families).is_err());
        let bad_families = BTreeSet::from(["e_scum".into(), "e_goburi".into()]);
        assert!(fan_out(&chunk(false), false, &bad_families).is_err());
    }
}
