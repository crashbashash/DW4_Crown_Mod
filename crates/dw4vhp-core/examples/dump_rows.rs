//! Print each enemy family's rows with HP, attack and rarity from the disc's table.
//! `dump_rows <iso> [model]` also prints all twelve stat columns of that model's rows.
use dw4vhp_core::{layout::Layout, rowgroup::ROW_GROUPS, table::EnemyTable};
fn main() {
    let mut args = std::env::args().skip(1);
    let iso = std::fs::read(args.next().expect("iso")).expect("read");
    let only = args.next();
    let t = EnemyTable::read(&iso, &Layout::retail()).expect("table");
    for g in ROW_GROUPS {
        if only.as_deref().is_some_and(|m| m != g.model) {
            continue;
        }
        if only.is_some() {
            println!("{}", g.model);
            for &r in g.rows {
                println!(
                    "  {r:3} hp {:6} stat {:?} crit {} para {} rare {}",
                    t.hp[r], t.stat[r], t.crit[r], t.para[r], t.rarity[r]
                );
            }
            continue;
        }
        print!("{:12} ", g.model);
        for &r in g.rows {
            print!("[{}:{}/{}/r{}] ", r, t.hp[r], t.stat[r][0], t.rarity[r]);
        }
        println!();
    }
}
