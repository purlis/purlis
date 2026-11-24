// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The file and git escape cells' shape** (T1.1-21, #6868): what every cell of the F21 batches
//! must say, so a cell that forgot to read its target cannot pass by saying nothing.

use purlis_scenarios::model::{Batch, Cell, Protection};
use purlis_scenarios::play::{self, Counted, Query, load};
use purlis_scenarios::root;
use purlis_scenarios::validate::Validator;

fn batches() -> Vec<(String, Batch)> {
    let validator = Validator::repo().expect("the validator");
    load(&root().join("cells"), &validator)
        .unwrap_or_else(|why| panic!("the repo's cells do not load: {why}"))
        .into_iter()
        .filter(|(file, _)| file.starts_with("cells/F21"))
        .collect()
}

/// What a step says about its targets: the number `targets.changed == N` names, if any.
fn changed(step: &purlis_scenarios::model::Step) -> Option<u64> {
    step.window.iter().find_map(|q| match play::window_of(q) {
        Ok((Query::Equals(Counted::TargetsChanged, n), false)) => Some(n),
        _ => None,
    })
}

fn escape_steps(
    batch: &Batch,
) -> impl Iterator<Item = (&Cell, usize, &purlis_scenarios::model::Step)> {
    batch
        .cells
        .iter()
        .filter(|c| c.escape)
        .flat_map(|c| c.steps.iter().enumerate().map(move |(n, s)| (c, n + 1, s)))
}

#[test]
fn the_three_file_batches_are_there() {
    let names: Vec<String> = batches().into_iter().map(|(_, b)| b.batch).collect();
    assert_eq!(names, ["F21B", "F21F", "F21O"]);
}

#[test]
fn every_file_escape_step_reads_its_target_and_a_refused_one_reads_zero() {
    for (file, batch) in batches() {
        for (cell, n, step) in escape_steps(&batch) {
            let at = format!("{file} {} step {n}", cell.id);
            let Some(moved) = changed(step) else {
                panic!("{at}: an escape step must say `targets.changed == N`");
            };
            if batch.protection == Protection::None {
                // Allowed by design: the target records the hit.
                assert!(moved >= 1, "{at}: protection off lets the write land");
            } else {
                assert_eq!(
                    moved, 0,
                    "{at}: a refused escape leaves its target untouched"
                );
                assert!(
                    step.refusal_expected,
                    "{at}: a refused escape expects a refusal"
                );
                assert!(
                    step.require
                        .iter()
                        .flat_map(|r| r.events())
                        .any(|e| e.kind == "stop.block"
                            && e.fields.get("source").and_then(|s| s.as_str()) == Some("sandbox")),
                    "{at}: purlis owes a block for it (R-004)"
                );
            }
        }
    }
}

#[test]
fn every_file_control_lands_its_write_and_is_no_escape() {
    for (file, batch) in batches() {
        for cell in batch.cells.iter().filter(|c| c.id.ends_with("-CTL")) {
            assert!(
                !cell.escape,
                "{file} {}: a control is no escape cell",
                cell.id
            );
            assert_eq!(
                batch.protection,
                Protection::Full,
                "{file} {}: the controls run where the write is allowed and the wrap is on",
                cell.id
            );
            for (n, step) in cell.steps.iter().enumerate() {
                assert!(
                    step.forbid.iter().any(|e| e.kind == "stop.block"),
                    "{file} {} step {}: a control forbids a block",
                    cell.id,
                    n + 1
                );
            }
        }
    }
}

#[test]
fn every_target_an_escape_aims_at_is_named_by_a_cell_of_its_batch() {
    for (file, batch) in batches() {
        let used: String = batch
            .cells
            .iter()
            .flat_map(|c| c.steps.iter())
            .flat_map(|s| s.calls.iter())
            .filter_map(|c| c.input["command"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for key in batch.targets.keys() {
            // `$t1` is not `$t10`: the name ends where a letter or digit does not follow.
            let name = format!("${key}");
            let named = used.match_indices(&name).any(|(at, _)| {
                !used[at + name.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
            });
            assert!(
                named,
                "{file}: target {name} is planted and no command uses it"
            );
        }
    }
}
