//! Print each enemy family's rows with HP, attack and rarity from the disc's table.
use dw4vhp_core::{layout::Layout, rowgroup::ROW_GROUPS, table::EnemyTable};
fn main() {
    let iso = std::fs::read(std::env::args().nth(1).expect("iso")).expect("read");
    let t = EnemyTable::read(&iso, &Layout::retail()).expect("table");
    for g in ROW_GROUPS {
        print!("{:12} ", g.model);
        for &r in g.rows {
            print!("[{}:{}/{}/r{}] ", r, t.hp[r], t.stat[r][0], t.rarity[r]);
        }
        println!();
    }
}
