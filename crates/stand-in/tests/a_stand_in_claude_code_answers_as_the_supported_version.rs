// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A stand-in Claude Code answers `--version` as the Claude Code purlis supports
//! (`tests/fixtures/harness-versions/claude.txt`), and the window's scenarios' stand-in
//! (`app/e2e/harness.ts`) as this crate's does. Pool setup's run installs its stand-in under the
//! version file's version and pins the pool's profile to the stand-in's: two versions that
//! differ leave the runner without the harness its profile pins, and Pool setup asks for a real
//! sign-in (#6934, the bump to 2.1.325, T1.1-03).

const SUPPORTED: &str = include_str!("../../../tests/fixtures/harness-versions/claude.txt");
const E2E_HARNESS: &str = include_str!("../../../app/e2e/harness.ts");

#[test]
fn a_stand_in_claude_code_answers_as_the_supported_version() {
    assert_eq!(stand_in::CLAUDE_CODE_VERSION, SUPPORTED.trim());
    let e2e = format!(
        "export const CLAUDE_CODE_VERSION = \"{}\";",
        stand_in::CLAUDE_CODE_VERSION
    );
    assert!(
        E2E_HARNESS.contains(&e2e),
        "app/e2e/harness.ts does not say `{e2e}`"
    );
}
