// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The routes `api` never reaches on the forge's own host** (#3894): those that do what a
//! refused command does, so `api` is no way round [`super::Forge::refused`]. Each route is held
//! whatever the method, as the command it stands for is refused whole. The audit that drew them
//! is `docs/forge-tool-refusals.md`.

use super::Forge;

/// GitHub's REST routes, each a run of path segments that refuses the route and every path
/// under it, or the route alone where it ends with `$`; `*` stands for any one segment, and a
/// segment matches a name with a format after a dot (`keys.json`). A path through `repositories/<id>` or
/// `organizations/<id>` is read as `repos/…` or `orgs/…` first.
const GH_ROUTES: &[&str] = &[
    "user/keys",
    "user/ssh_signing_keys",
    "user/gpg_keys",
    "repos/*/*/keys",
    "repos/*/*/actions/secrets",
    "repos/*/*/actions/organization-secrets",
    "repos/*/*/environments/*/secrets",
    "repos/*/*/codespaces/secrets",
    "repos/*/*/codespaces/organization-secrets",
    "repos/*/*/dependabot/secrets",
    "orgs/*/actions/secrets",
    "orgs/*/codespaces/secrets",
    "orgs/*/dependabot/secrets",
    "orgs/*/private-registries",
    "user/codespaces/secrets",
    "repos/*/*/actions/runners/registration-token",
    "repos/*/*/actions/runners/remove-token",
    "repos/*/*/actions/runners/generate-jitconfig",
    "orgs/*/actions/runners/registration-token",
    "orgs/*/actions/runners/remove-token",
    "orgs/*/actions/runners/generate-jitconfig",
    "enterprises/*/actions/runners/registration-token",
    "enterprises/*/actions/runners/remove-token",
    "enterprises/*/actions/runners/generate-jitconfig",
    "orgs/*/personal-access-tokens",
    "orgs/*/personal-access-token-requests",
    "authorizations",
    "applications/*/token",
    "applications/*/grant",
    "applications/*/grants",
    "app/installations/*/access_tokens",
    "credentials/revoke",
    "orgs/*/credential-authorizations",
    "installation/token",
    "app-manifests/*/conversions",
    "admin/keys",
    "admin/users/*/authorizations",
];

/// GitLab's REST routes (API v4), read as [`GH_ROUTES`] are.
const GLAB_ROUTES: &[&str] = &[
    "user/keys",
    "users/*/keys",
    "user/gpg_keys",
    "users/*/gpg_keys",
    "deploy_keys",
    "projects/*/deploy_keys",
    "deploy_tokens",
    "projects/*/deploy_tokens",
    "groups/*/deploy_tokens",
    "personal_access_tokens",
    "user/personal_access_tokens",
    "users/*/personal_access_tokens",
    "users/*/impersonation_tokens",
    "groups/*/service_accounts/*/personal_access_tokens",
    "projects/*/access_tokens",
    "groups/*/access_tokens",
    "projects/*/variables",
    "groups/*/variables",
    "admin/ci/variables",
    "projects/*/pipeline_schedules",
    "projects/*/pipelines/*/variables",
    "projects/*/terraform/state",
    "projects/*/secure_files",
    "projects/*/triggers",
    "projects/*/cluster_agents/*/tokens",
    "runners/reset_registration_token",
    "projects/*/runners/reset_registration_token",
    "groups/*/runners/reset_registration_token",
    "runners/reset_authentication_token",
    "runners/*/reset_authentication_token",
    "user/runners",
    "applications",
    "user/support_pin",
    "application/settings",
    // A project's and a group's own page show its runner registration token to a maintainer.
    "projects/*$",
    "groups/*$",
];

/// The fields of GitHub's GraphQL schema that show or make a credential: a query naming one
/// never runs. Its schema has no mutation that adds or removes a key or a secret.
const GH_GRAPHQL_FIELDS: &[&str] = &[
    "tempclonetoken",
    "recoverycodes",
    "verificationtoken",
    "verifiabledomaintoken",
];

/// **Each way a server may read the path `arg` names**, if it names one: none for an argument
/// that is no endpoint (an option, a field's `key=value`, a header). The query and fragment are
/// cut; the path is split at `/` and `\\` and then percent-decoded, and decoded and then split;
/// each with its `.` and `..` segments as written and as resolved. Empty segments drop, every
/// segment is lower-cased, a leading `api/v3` or `api/v4` drops, and `repositories/<id>`,
/// `organizations/<id>` read as `repos/<id>/<id>`, `orgs/<id>`.
fn readings(arg: &str) -> Vec<Vec<String>> {
    if arg.starts_with('-') {
        return Vec::new();
    }
    let path = arg.split(['?', '#']).next().unwrap_or_default();
    if path.contains('=') || path.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let decode = |one: &str| {
        percent_encoding::percent_decode_str(one)
            .decode_utf8_lossy()
            .to_lowercase()
    };
    let split_first: Vec<String> = path.split(['/', '\\']).map(decode).collect();
    let decoded_first: Vec<String> = decode(path).split(['/', '\\']).map(str::to_owned).collect();
    let mut out = Vec::new();
    for segments in [split_first, decoded_first] {
        let written: Vec<String> = segments
            .iter()
            .filter(|one| !one.is_empty())
            .cloned()
            .collect();
        let mut resolved: Vec<String> = Vec::new();
        for one in &written {
            match one.as_str() {
                "." => {}
                ".." => {
                    resolved.pop();
                }
                _ => resolved.push(one.clone()),
            }
        }
        out.push(read_as_routed(written));
        out.push(read_as_routed(resolved));
    }
    out
}

/// `segments` without a leading `api/v3` or `api/v4`, and with `repositories/<id>` and
/// `organizations/<id>` read as the routes they stand for.
fn read_as_routed(mut segments: Vec<String>) -> Vec<String> {
    if segments.len() >= 2 && segments[0] == "api" && (segments[1] == "v3" || segments[1] == "v4") {
        segments.drain(..2);
    }
    match segments.first().map(String::as_str) {
        Some("repositories") if segments.len() >= 2 => {
            let id = segments[1].clone();
            segments.splice(..2, ["repos".to_owned(), id.clone(), id]);
        }
        Some("organizations") => segments[0] = "orgs".to_owned(),
        _ => {}
    }
    segments
}

/// Whether `route`'s segments lead `segments`, or are all of them where it ends with `$`.
fn under(route: &str, segments: &[String]) -> bool {
    let (route, alone) = match route.strip_suffix('$') {
        Some(route) => (route, true),
        None => (route, false),
    };
    let route: Vec<&str> = route.split('/').collect();
    let fits = if alone {
        route.len() == segments.len()
    } else {
        route.len() <= segments.len()
    };
    fits && route.iter().zip(segments).all(|(want, one)| {
        *want == "*"
            || *want == one
            || one
                .strip_prefix(*want)
                .is_some_and(|format| format.starts_with('.'))
    })
}

/// **Why `forge`'s `api` run with `args` (its command first) never runs**, if it does not: an
/// endpoint read, any way a server may read it, as one under a route of the forge's list; GitLab's GraphQL, which
/// reaches its CI/CD variables' values and makes runner and agent tokens; or GitHub's, where the
/// query names a credential's field or is read from a file or standard input, so is never seen.
pub(super) fn refused(forge: Forge, args: &[String]) -> Option<String> {
    let routes = match forge {
        Forge::Gh => GH_ROUTES,
        Forge::Glab => GLAB_ROUTES,
    };
    let mut graphql = false;
    for arg in args.iter().skip(1) {
        for segments in readings(arg) {
            if let Some(route) = routes.iter().find(|route| under(route, &segments)) {
                let route = route.trim_end_matches('$');
                return Some(format!(
                    "{route} shows or changes the forge's credentials or secrets, as a refused \
                     command does"
                ));
            }
            graphql |= segments == ["graphql"];
        }
    }
    if !graphql {
        return None;
    }
    if forge == Forge::Glab {
        return Some(
            "GitLab's GraphQL reaches CI/CD variables' values and makes runner and agent tokens"
                .to_owned(),
        );
    }
    let unseen = args
        .iter()
        .any(|arg| arg == "--input" || arg.starts_with("--input=") || arg.contains("=@"));
    if unseen {
        return Some(
            "its query is read from a file or standard input, so it is not seen".to_owned(),
        );
    }
    args.iter()
        .map(|arg| arg.to_ascii_lowercase())
        .find_map(|arg| GH_GRAPHQL_FIELDS.iter().find(|field| arg.contains(*field)))
        .map(|field| format!("its query asks for {field}, a credential"))
}
