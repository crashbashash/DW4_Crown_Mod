//! Cover compatible families before balancing the remaining encounter population.
use super::*;

pub(super) struct Slot {
    pub name: String,
    pub options: Vec<String>,
    pub weight: u64,
}

fn cover(
    model: &str,
    slots: &[Slot],
    matched: &mut BTreeMap<usize, String>,
    visited: &mut BTreeSet<usize>,
) -> bool {
    for (index, slot) in slots.iter().enumerate() {
        if !slot.options.iter().any(|m| m == model) || !visited.insert(index) {
            continue;
        }
        let previous = matched.get(&index).cloned();
        if previous.is_none_or(|m| cover(&m, slots, matched, visited)) {
            matched.insert(index, model.to_string());
            return true;
        }
    }
    false
}

pub(super) fn assign(
    mut slots: Vec<Slot>,
    fixed: &BTreeMap<String, String>,
    weights: &BTreeMap<String, u64>,
    rng: &mut Rng,
) -> BTreeMap<String, String> {
    rng.shuffle(&mut slots);
    let mut load: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    for (name, model) in fixed {
        let entry = load.entry(model.clone()).or_default();
        entry.0 += weights.get(name).copied().unwrap_or(1);
        entry.1 += 1;
    }
    let mut families: Vec<_> = slots
        .iter()
        .flat_map(|slot| slot.options.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|m| !load.contains_key(m))
        .collect();
    rng.shuffle(&mut families);
    // Scarce families get first choice. Augmenting paths avoid spending the
    // only compatible group on a family that could use a different group.
    families.sort_by_key(|m| slots.iter().filter(|s| s.options.contains(m)).count());
    let mut matched = BTreeMap::new();
    for model in families {
        cover(&model, &slots, &mut matched, &mut BTreeSet::new());
    }
    let mut result = BTreeMap::new();
    for (&index, model) in &matched {
        let slot = &slots[index];
        let entry = load.entry(model.clone()).or_default();
        entry.0 += slot.weight;
        entry.1 += 1;
        result.insert(slot.name.clone(), model.clone());
    }
    // Large groups first makes the population balance less sensitive to a
    // stage having a mixture of singletons and large refill waves.
    let mut remaining: Vec<_> = slots
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !matched.contains_key(index))
        .map(|(_, slot)| slot)
        .collect();
    remaining.sort_by_key(|slot| std::cmp::Reverse(slot.weight));
    for mut slot in remaining {
        rng.shuffle(&mut slot.options);
        if let Some(model) = slot
            .options
            .into_iter()
            .min_by_key(|model| load.get(model).copied().unwrap_or_default())
        {
            let entry = load.entry(model.clone()).or_default();
            entry.0 += slot.weight;
            entry.1 += 1;
            result.insert(slot.name, model);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(name: &str, options: &[&str], weight: u64) -> Slot {
        Slot {
            name: name.into(),
            options: options.iter().map(|s| s.to_string()).collect(),
            weight,
        }
    }

    #[test]
    fn four_families_cover_and_balance_twenty_groups_for_many_seeds() {
        for seed in 0..100 {
            let slots = (0..20)
                .map(|i| slot(&i.to_string(), &["a", "b", "c", "d"], 4))
                .collect();
            let picks = assign(
                slots,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &mut Rng::new(seed, 1),
            );
            for model in ["a", "b", "c", "d"] {
                assert_eq!(picks.values().filter(|m| *m == model).count(), 5);
            }
        }
    }

    #[test]
    fn coverage_respects_scarcity_and_reassigns_earlier_matches() {
        for seed in 0..100 {
            let picks = assign(
                vec![
                    slot("one", &["a", "b"], 1),
                    slot("two", &["a", "c"], 1),
                    slot("three", &["c", "d"], 1),
                    slot("four", &["d"], 1),
                ],
                &BTreeMap::new(),
                &BTreeMap::new(),
                &mut Rng::new(seed, 1),
            );
            assert_eq!(picks.values().collect::<BTreeSet<_>>().len(), 4);
            assert_eq!(picks["one"], "b");
            assert_eq!(picks["four"], "d");
        }
    }

    #[test]
    fn existing_intro_counts_toward_coverage_and_small_stages_stay_compatible() {
        let fixed = BTreeMap::from([("intro".into(), "a".into())]);
        let picks = assign(
            vec![slot("one", &["a", "b"], 1), slot("two", &["a", "c"], 1)],
            &fixed,
            &BTreeMap::new(),
            &mut Rng::new(17, 1),
        );
        assert_eq!(picks["one"], "b");
        assert_eq!(picks["two"], "c");
    }

    #[test]
    fn population_balance_is_reproducible_with_large_and_small_groups() {
        for seed in 0..30 {
            let slots = || {
                (0..16)
                    .map(|i| {
                        slot(
                            &i.to_string(),
                            &["a", "b", "c", "d"],
                            if i < 4 { 10 } else { 1 },
                        )
                    })
                    .collect()
            };
            let a = assign(
                slots(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &mut Rng::new(seed, 1),
            );
            let b = assign(
                slots(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &mut Rng::new(seed, 1),
            );
            assert_eq!(a, b, "same seed, same groups");
            for model in ["a", "b", "c", "d"] {
                let population: u64 = a
                    .iter()
                    .filter(|(_, m)| *m == model)
                    .map(|(name, _)| {
                        if name.parse::<usize>().unwrap() < 4 {
                            10
                        } else {
                            1
                        }
                    })
                    .sum();
                assert_eq!(population, 13);
            }
        }
    }
}
