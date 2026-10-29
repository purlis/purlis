// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A GitHub App's token as a persona's tool (#2053, E1 and E2): what is read, what is refused,
//! the line an Allow is bound to, and which keys are marked so they are never handed over raw.

use super::mint::{self, Mint};
use super::{Asked, Handed, Ready, Tool, declared, keyed};

fn read(text: &str) -> (Vec<Tool>, Vec<String>) {
    let value: toml::Value = toml::from_str(text).expect("TOML");
    let (tools, refused) = keyed(
        value.get("tools"),
        "sandbox.personas.dev",
        "charter.toml",
        &["sandbox", "personas", "dev"],
    );
    (tools, refused.into_iter().map(|one| one.why).collect())
}

fn gh() -> Tool {
    Tool {
        mint: Some(Mint {
            app: "Iv23liExample".to_owned(),
        }),
        ..Tool::one("gh", "forge", "GH_TOKEN", Handed::Env, "app-key")
    }
}

#[test]
fn a_github_app_tool_is_handed_its_token_in_gh_token_as_env() {
    let (tools, refused) = read(
        r#"tools = { gh = { vault = "forge", mint = "github-app", app = "Iv23liExample", key = "app-key" } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(tools, vec![gh()]);
}

#[test]
fn a_github_app_tool_takes_no_hand_over_of_its_own_and_one_kind_alone() {
    for (line, said) in [
        (
            r#"gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "k", as = "GH_TOKEN file" }"#,
            "takes no as",
        ),
        (
            r#"gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "k", browser = "https://x.test/" }"#,
            "takes no browser",
        ),
        (
            r#"gh = { vault = "forge", mint = "gitlab", app = "Iv1.abc", key = "k" }"#,
            "is not a kind purlis mints",
        ),
        (
            r#"gh = { vault = "forge", mint = "github-app", key = "k" }"#,
            "app names no GitHub App",
        ),
        (
            r#"gh = { vault = "forge", mint = "github-app", app = "Iv1 abc", key = "k" }"#,
            "is not a GitHub App's client id",
        ),
        (
            r#"gh = { vault = "forge", app = "Iv1.abc", key = "k" }"#,
            "mint is not a kind",
        ),
        (
            r#"gh = { vault = "forge", mint = "github-app", app = "Iv1.abc" }"#,
            "names no key of the vault",
        ),
        (
            r#"gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", values = { A = { key = "k", as = "env" } } }"#,
            "has both values and mint",
        ),
    ] {
        let (tools, refused) = read(&format!("tools = {{ {line} }}"));
        assert_eq!(tools, Vec::new(), "{line}");
        assert!(
            refused.len() == 1 && refused[0].contains(said),
            "{line}: {refused:?}"
        );
    }
}

#[test]
fn its_line_names_the_app_so_another_app_asks_again() {
    assert_eq!(
        gh().whole(),
        "gh with a token GitHub App Iv23liExample mints from \"app-key\" in vault forge as \
         GH_TOKEN env"
    );
    let other = Tool {
        mint: Some(Mint {
            app: "Iv23liOther".to_owned(),
        }),
        ..gh()
    };
    assert_ne!(gh().whole(), other.whole());
    // Not the line a plain tool handed the key itself would have.
    assert_ne!(
        gh().whole(),
        Tool::one("gh", "forge", "GH_TOKEN", Handed::Env, "app-key").whole()
    );
}

#[test]
fn a_chat_runs_it_with_exactly_its_hand_over() {
    let armed = vec![Ready {
        tool: gh(),
        program: "/usr/local/bin/gh".into(),
    }];
    let env = vec!["GH_TOKEN=app-key".to_owned()];
    let asked = Asked {
        vault: "forge",
        env: &env,
        file: &[],
        dotenv: &[],
        program: "gh",
    };
    assert_eq!(
        declared(&armed, &[], "dev", &asked).map(|ready| ready.tool.mint.clone()),
        Ok(Some(Mint {
            app: "Iv23liExample".to_owned()
        }))
    );
    assert_eq!(
        gh().secret_exec(std::path::Path::new("/usr/local/bin/gh")),
        [
            "secret",
            "exec",
            "forge",
            "--env",
            "GH_TOKEN=app-key",
            "--",
            "/usr/local/bin/gh"
        ]
    );
}

#[test]
fn every_key_a_mint_line_names_is_marked_even_on_a_line_that_is_not_taken() {
    let cfg: toml::Table = toml::from_str(
        r#"
[sandbox.personas.dev]
tools = { gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "app-key" },
          kubectl = { vault = "forge", as = "KUBECONFIG file", key = "kubeconfig" } }

[sandbox.personas.ops]
tools = { gh = { vault = "ops", mint = "nonsense", key = "other-key", as = "X env" } }
"#,
    )
    .unwrap();
    assert_eq!(
        mint::marked_in(&cfg),
        vec![
            ("forge".to_owned(), "app-key".to_owned()),
            ("ops".to_owned(), "other-key".to_owned()),
        ]
    );
}

#[test]
fn a_marked_key_is_refused_raw_and_another_is_not() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        crate::names::manifest(dir.path()),
        r#"
[sandbox.personas.dev]
tools = { gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "app-key" } }
"#,
    )
    .unwrap();
    let refused = mint::refused_raw(dir.path(), "forge", "app-key").expect("refused");
    assert!(refused.contains("never hands it over"), "{refused}");
    assert_eq!(mint::refused_raw(dir.path(), "forge", "kubeconfig"), None);
    assert_eq!(mint::refused_raw(dir.path(), "other", "app-key"), None);
}
