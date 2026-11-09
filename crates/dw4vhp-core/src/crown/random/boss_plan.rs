//! Seeded boss assignment. Shuffle a fixed encounter inventory rather
//! than rolling each arena separately. Patrols must be the final ordinary-enemy
//! choices for this stage and difficulty, not its original resident resources.
use super::*;

/// Both ordinary Mecha Rogue families are valid bosses when they do not also
/// occur in that arena's selected patrols. World bosses never enter this pool.
pub(super) const MODELS: &[&str] = &[
    "e_blosso",
    "e_mammos",
    "e_skulgrey",
    "e_anomalo",
    "e_tonogeko",
    "e_mecha4",
    "e_diabolo",
    "e_mecha5",
    "e_ldknight",
];

pub(super) struct Slot {
    pub id: String,
    pub original: String,
    pub patrols: BTreeSet<String>,
    /// Replacement families whose resource and encounter checks passed.
    pub compatible: BTreeSet<String>,
}

impl Slot {
    fn allows(&self, model: &str) -> bool {
        // Keeping an authored encounter is the safe fallback. A replacement
        // must pass compatibility and cannot duplicate any selected patrol.
        model == self.original || (self.compatible.contains(model) && !self.patrols.contains(model))
    }
}

struct Edge {
    to: usize,
    reverse: usize,
    available: bool,
    cost: i32,
}

fn connect(graph: &mut [Vec<Edge>], from: usize, to: usize, cost: i32) {
    let forward = graph[from].len();
    let reverse = graph[to].len();
    graph[from].push(Edge {
        to,
        reverse,
        available: true,
        cost,
    });
    graph[to].push(Edge {
        to: from,
        reverse: forward,
        available: false,
        cost: -cost,
    });
}

/// Assign exactly the supplied original inventory for one difficulty. The
/// matching can revise earlier choices to accommodate scarce families. Among
/// valid assignments it minimizes unchanged bosses, with seeded tie ordering.
/// Callers must commit the complete plan atomically: rerolling or falling back
/// an individual arena afterwards would invalidate the encounter counts.
pub(super) fn assign(slots: &[Slot], rng: &mut Rng) -> Result<BTreeMap<String, String>> {
    let mut order = (0..slots.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| slots[a].id.cmp(&slots[b].id));
    need(
        order.windows(2).all(|p| slots[p[0]].id != slots[p[1]].id),
        "duplicate boss encounter",
    )?;
    need(
        slots.iter().all(|s| MODELS.contains(&s.original.as_str())),
        "unsupported boss inventory",
    )?;
    let mut tokens = slots.iter().map(|s| s.original.clone()).collect::<Vec<_>>();
    tokens.sort();
    rng.shuffle(&mut order);
    rng.shuffle(&mut tokens);
    let count = slots.len();
    let sink = 2 * count + 1;
    let mut graph = (0..=sink).map(|_| Vec::new()).collect::<Vec<_>>();
    for (token, model) in tokens.iter().enumerate() {
        connect(&mut graph, 0, token + 1, 0);
        for (position, &index) in order.iter().enumerate() {
            if slots[index].allows(model) {
                connect(
                    &mut graph,
                    token + 1,
                    count + position + 1,
                    i32::from(*model == slots[index].original),
                );
            }
        }
    }
    for position in 0..count {
        connect(&mut graph, count + position + 1, sink, 0);
    }
    for _ in 0..count {
        let mut distance = vec![i32::MAX; graph.len()];
        let mut previous = vec![None; graph.len()];
        distance[0] = 0;
        // Reverse edges can have negative costs. Relaxing the residual graph
        // lets a new scarce assignment move earlier bosses instead of giving
        // one family an extra occurrence or silently dropping another family.
        for _ in 1..graph.len() {
            let mut changed = false;
            for from in 0..graph.len() {
                if distance[from] == i32::MAX {
                    continue;
                }
                for (index, edge) in graph[from].iter().enumerate() {
                    let next = distance[from] + edge.cost;
                    if edge.available && next < distance[edge.to] {
                        distance[edge.to] = next;
                        previous[edge.to] = Some((from, index));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        need(
            previous[sink].is_some(),
            "boss inventory cannot fit its encounters",
        )?;
        let mut to = sink;
        while to != 0 {
            let (from, index) = previous[to].ok_or_else(|| bad("incomplete boss assignment"))?;
            let reverse = graph[from][index].reverse;
            graph[from][index].available = false;
            graph[to][reverse].available = true;
            to = from;
        }
    }
    let mut result = BTreeMap::new();
    for (token, model) in tokens.iter().enumerate() {
        for edge in &graph[token + 1] {
            if edge.to > count && edge.to < sink && !edge.available {
                let slot = &slots[order[edge.to - count - 1]];
                need(
                    result.insert(slot.id.clone(), model.clone()).is_none(),
                    "boss slot assigned twice",
                )?;
            }
        }
    }
    need(result.len() == count, "missing boss assignment")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, original: &str, patrols: &[&str]) -> Slot {
        Slot {
            id: id.into(),
            original: original.into(),
            patrols: patrols.iter().map(|s| s.to_string()).collect(),
            compatible: MODELS.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn counts(models: impl IntoIterator<Item = String>) -> BTreeMap<String, usize> {
        let mut result = BTreeMap::new();
        for model in models {
            *result.entry(model).or_default() += 1;
        }
        result
    }

    #[test]
    fn mr05_can_replace_a_boss_only_where_it_is_absent_from_selected_patrols() {
        for seed in 0..100 {
            let slots = [
                slot("cave", "e_blosso", &["e_mecha5"]),
                slot("cliff", "e_mammos", &[]),
                slot("mine", "e_mecha5", &[]),
            ];
            let chosen = assign(&slots, &mut Rng::new(seed, 1)).unwrap();
            assert_eq!(chosen["cave"], "e_mammos");
            assert_eq!(chosen["cliff"], "e_mecha5");
            assert_eq!(chosen["mine"], "e_blosso");
        }
    }

    #[test]
    fn every_boss_family_is_excluded_from_replacements_where_it_is_a_patrol() {
        for &model in MODELS {
            let original = if model == "e_blosso" {
                "e_mammos"
            } else {
                "e_blosso"
            };
            let target = slot("arena", original, &[model]);
            assert!(!target.allows(model), "{model} duplicates a patrol");
            assert!(slot("arena", original, &[]).allows(model));
        }
    }

    #[test]
    fn a_repeated_boss_keeps_its_exact_count_across_many_seeds_and_difficulties() {
        let mut variants = BTreeSet::new();
        for difficulty in 0..3 {
            for seed in 0..200 {
                let mut slots = vec![
                    slot("blossomon-one", "e_blosso", &[]),
                    slot("blossomon-two", "e_blosso", &[]),
                    slot("mammothmon", "e_mammos", &["e_mecha5"]),
                    slot("mr05", "e_mecha5", &[]),
                    slot("lordknightmon-one", "e_ldknight", &[]),
                ];
                for index in 0..difficulty {
                    slots.push(slot(
                        &format!("lordknightmon-extra-{index}"),
                        "e_ldknight",
                        &[],
                    ));
                }
                let chosen = assign(&slots, &mut Rng::new(seed, difficulty)).unwrap();
                assert_eq!(
                    counts(chosen.values().cloned()),
                    counts(slots.iter().map(|s| s.original.clone()))
                );
                assert_eq!(chosen.values().filter(|m| *m == "e_blosso").count(), 2);
                for s in &slots {
                    assert!(chosen[&s.id] == s.original || !s.patrols.contains(&chosen[&s.id]));
                }
                assert_eq!(
                    chosen,
                    assign(&slots, &mut Rng::new(seed, difficulty)).unwrap()
                );
                slots.reverse();
                assert_eq!(
                    chosen,
                    assign(&slots, &mut Rng::new(seed, difficulty)).unwrap()
                );
                variants.insert(chosen.into_iter().collect::<Vec<_>>());
            }
        }
        assert!(variants.len() > 3, "seeds should change boss locations");
    }

    #[test]
    fn incompatible_changes_retain_the_inventory_instead_of_rerolling_a_single_arena() {
        let slots = [
            slot("cave-one", "e_blosso", &["e_mecha5"]),
            slot("cave-two", "e_blosso", &["e_mecha5"]),
            slot("mine", "e_mecha5", &[]),
        ];
        let chosen = assign(&slots, &mut Rng::new(1520, 1)).unwrap();
        assert_eq!(
            chosen,
            slots
                .iter()
                .map(|s| (s.id.clone(), s.original.clone()))
                .collect()
        );
    }

    #[test]
    fn constrained_matching_agrees_with_an_exhaustive_small_inventory() {
        fn best(slots: &[Slot], at: usize, remaining: &mut [bool], models: &[String]) -> usize {
            if at == slots.len() {
                return 0;
            }
            let mut result = usize::MAX;
            for token in 0..models.len() {
                if !remaining[token] || !slots[at].allows(&models[token]) {
                    continue;
                }
                remaining[token] = false;
                let rest = best(slots, at + 1, remaining, models);
                if rest != usize::MAX {
                    result = result.min(rest + usize::from(models[token] == slots[at].original));
                }
                remaining[token] = true;
            }
            result
        }
        for seed in 0..100 {
            let mut setup = Rng::new(seed, 7);
            let mut slots = vec![
                slot("a", "e_blosso", &[]),
                slot("b", "e_blosso", &[]),
                slot("c", "e_mammos", &[]),
                slot("d", "e_mecha5", &[]),
                slot("e", "e_ldknight", &[]),
            ];
            for s in &mut slots {
                for model in ["e_blosso", "e_mammos", "e_mecha5", "e_ldknight"] {
                    if setup.next().is_multiple_of(3) {
                        s.patrols.insert(model.into());
                    }
                    if setup.next().is_multiple_of(4) {
                        s.compatible.remove(model);
                    }
                }
            }
            let models = slots.iter().map(|s| s.original.clone()).collect::<Vec<_>>();
            let expected = best(&slots, 0, &mut vec![true; slots.len()], &models);
            let chosen = assign(&slots, &mut Rng::new(seed, 9)).unwrap();
            assert_eq!(
                chosen
                    .iter()
                    .filter(|(id, model)| {
                        slots.iter().find(|s| &s.id == *id).unwrap().original == **model
                    })
                    .count(),
                expected
            );
            assert_eq!(counts(chosen.values().cloned()), counts(models));
            assert!(slots.iter().all(|s| s.allows(&chosen[&s.id])));
        }
    }

    #[test]
    fn world_bosses_and_unknown_encounters_cannot_enter_the_inventory() {
        for model in [
            "f_apocaly",
            "f_belvam",
            "f_luce",
            "f_luce2",
            "f_devl1",
            "f_devl1b",
            "f_devl22",
            "f_devl3",
            "e_mecha4bs",
        ] {
            assert!(assign(&[slot("world", model, &[])], &mut Rng::new(1, 1)).is_err());
        }
        assert!(assign(&[], &mut Rng::new(1, 1)).unwrap().is_empty());
        assert!(assign(
            &[slot("same", "e_blosso", &[]), slot("same", "e_mammos", &[])],
            &mut Rng::new(1, 1)
        )
        .is_err());
    }
}
