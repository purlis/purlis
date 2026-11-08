// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The CI grader (#5727 of spec #5714), from outside: what a change grades, and a grade run
//! against a stand-in harness that replays recorded output, so no test calls a model.

#[path = "fixture.rs"]
mod fixture;

mod a_built_ins_trigger_tests_have_the_counts_sources_and_bar_a_score_needs;
mod a_change_grades_only_the_built_ins_it_touches;
mod a_grade_runs_each_test_among_the_crowd_and_bare_and_says_its_scores;
mod codex_and_opencode_are_graded_as_a_chat_in_the_window_starts_them;
mod the_nightly_tracks_a_drop_per_skill_and_harness_and_bumps_the_pins_when_green;
mod the_optimiser_proposes_a_description_and_shows_both_scores;
mod without_its_key_the_grade_skips_and_says_why;
