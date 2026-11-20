// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The old routes that handed a chat a forge token are replaced by the person's sign-in** (ADR
//! 0128, "Migration"; only with the `forge-wiring` feature on). A project written for an older
//! build still loads: its `mint = "github-app"` line and its vault forge token run nothing and
//! say what to do, no chat is handed a forge token, and no GitHub App token is ever minted.

use purlis_core::forgelock;
use purlis_core::sandbox::tools::{self, replaced};

const OLD_PLANE: &str = r#"
[sandbox.personas.dev.tools]
gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "app-key" }
glab = { vault = "forge", as = "GITLAB_TOKEN env", key = "token" }
kubectl = { vault = "ops", as = "KUBECONFIG file", key = "kubeconfig" }

[sandbox.personas.dev.tools.aws]
vault = "cloud"
[sandbox.personas.dev.tools.aws.values.GH_TOKEN]
key = "k"
as = "env"
"#;

fn old_tools() -> (Vec<tools::Tool>, Vec<purlis_core::settings::Refusal>) {
    let cfg: toml::Table = OLD_PLANE.parse().unwrap();
    let persona = &cfg["sandbox"]["personas"]["dev"];
    tools::keyed(
        persona.get("tools"),
        "sandbox.personas.dev",
        "purlis.toml",
        &["sandbox", "personas", "dev"],
    )
}

#[test]
fn an_old_plane_still_loads_and_only_its_unrelated_tool_runs() {
    purlis_core::unsteered!();
    let (tools, _) = old_tools();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["kubectl"]);
}

#[test]
fn each_old_line_says_to_sign_in_and_to_delete_the_line() {
    purlis_core::unsteered!();
    let (_, refused) = old_tools();
    assert_eq!(refused.len(), 3, "{refused:?}");
    for one in &refused {
        assert!(
            one.why.contains("Sign in under Settings › Forges")
                && one.why.contains("delete this line"),
            "{}",
            one.why
        );
    }
}

#[test]
fn no_tool_a_chat_is_handed_holds_a_forge_token_or_a_mint() {
    purlis_core::unsteered!();
    let (tools, _) = old_tools();
    for tool in &tools {
        assert!(tool.mint.is_none(), "{} mints", tool.name);
        for value in &tool.values {
            assert!(
                !replaced::token_var(&value.var),
                "{} hands {}",
                tool.name,
                value.var
            );
        }
    }
}

#[test]
fn the_project_file_is_left_as_written_and_its_old_lines_are_found_for_doctor() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let file = purlis_core::names::manifest(dir.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, OLD_PLANE).unwrap();
    let found = replaced::found(dir.path());
    assert_eq!(found.len(), 3);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), OLD_PLANE);
}

#[test]
fn every_chat_is_on_the_locks_baseline_with_no_mint_to_claim_it() {
    purlis_core::unsteered!();
    assert!(forgelock::claimed(false, false));
}
