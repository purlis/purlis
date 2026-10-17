// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::engine::Size;

use super::Opening;

#[test]
fn a_default_opening_is_a_shell_with_nothing_set() {
    // #2453: what fixtures build from. A real start sets every field.
    let opening = Opening::default();
    assert_eq!(opening.program, None, "no program is the operator's shell");
    assert!(opening.args.is_empty() && opening.cwd.is_none());
    assert_eq!(opening.size, Size::MIN);
    assert!(opening.env.is_empty() && opening.env_strip.is_empty());
    assert!(opening.declared_env.is_empty() && opening.env_pass.is_empty());
    assert!(opening.harness.is_none() && opening.git_hooks.is_none());
    assert!(!opening.operator_shell);
    assert!(opening.handed.is_empty());
    assert!(opening.confinement.is_none() && opening.run.is_none());
}
