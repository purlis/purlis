use super::{Found, Kind, found_in, kind_of, sentence, token_var};

fn table(text: &str) -> toml::Table {
    text.parse().expect("test toml")
}

fn tool(text: &str) -> toml::map::Map<String, toml::Value> {
    table(text)
}

#[test]
fn a_github_app_mint_line_is_the_github_app_route() {
    let line = tool("vault = \"forge\"\nmint = \"github-app\"\napp = \"Iv1.abc\"\nkey = \"k\"");
    assert_eq!(kind_of(&line), Some(Kind::GithubApp));
}

#[test]
fn a_vault_value_in_a_forge_token_variable_is_the_vault_token_route() {
    let one = tool("vault = \"forge\"\nas = \"GITLAB_TOKEN env\"\nkey = \"k\"");
    assert_eq!(kind_of(&one), Some(Kind::VaultToken));
    let several = tool("vault = \"forge\"\n[values.gh_token]\nkey = \"k\"\nas = \"env\"");
    assert_eq!(kind_of(&several), Some(Kind::VaultToken));
}

#[test]
fn a_tool_handing_any_other_value_is_not_an_old_route() {
    let one = tool("vault = \"v\"\nas = \"KUBECONFIG file\"\nkey = \"k\"");
    assert_eq!(kind_of(&one), None);
}

#[test]
fn a_token_connection_variable_is_never_handed_to_a_chat_either() {
    assert!(token_var("PURLIS_FORGE_TOKEN__6769746875622e636f6d"));
    assert!(token_var("ci_job_token"));
    assert!(!token_var("NPM_TOKEN"));
}

#[test]
fn every_old_line_of_a_project_file_is_found_by_persona_and_tool() {
    let cfg = table(
        r#"
[sandbox.personas.dev.tools]
gh = { vault = "forge", mint = "github-app", app = "Iv1.abc", key = "k" }
glab = { vault = "forge", as = "GITLAB_TOKEN env", key = "t" }
kubectl = { vault = "v", as = "KUBECONFIG file", key = "c" }
"#,
    );
    assert_eq!(
        found_in(&cfg),
        vec![
            Found {
                persona: "dev".into(),
                tool: "gh".into(),
                kind: Kind::GithubApp
            },
            Found {
                persona: "dev".into(),
                tool: "glab".into(),
                kind: Kind::VaultToken
            },
        ]
    );
}

#[test]
fn what_each_old_line_says_names_the_step_and_leaves_the_vault_item_alone() {
    assert!(sentence(Kind::GithubApp).contains("Sign in under Settings › Forges"));
    assert!(sentence(Kind::VaultToken).contains("the vault item stays until you delete it"));
}
