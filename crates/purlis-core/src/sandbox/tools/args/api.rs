// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The routes `api` never reaches on the forge's own host** (#3894): those that do what a
//! refused command does, so `api` is no way round [`super::Forge::refused`]. Each route is held
//! whatever the method, as the command it stands for is refused whole. The audit that drew them
//! is `docs/forge-tool-refusals.md`.

use super::Forge;

/// GitHub's REST routes, each a run of path segments that refuses the route and every path
/// under it; `*` stands for any one segment. A path through `repositories/<id>` or
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
    "projects/*/pipeline_schedules/*/variables",
    "projects/*/secure_files",
    "projects/*/triggers",
    "projects/*/cluster_agents/*/tokens",
    "runners/reset_registration_token",
    "projects/*/runners/reset_registration_token",
    "groups/*/runners/reset_registration_token",
    "runners/*/reset_authentication_token",
    "user/runners",
    "applications",
    "user/support_pin",
];

/// The fields of GitHub's GraphQL schema that show or make a credential: a query naming one
/// never runs. Its schema has no mutation that adds or removes a key or a secret.
const GH_GRAPHQL_FIELDS: &[&str] = &[
    "tempclonetoken",
    "recoverycodes",
    "verificationtoken",
    "verifiabledomaintoken",
];

/// What a path reads as: its segments, or that one of them is a dot segment, which a server may
/// resolve to a route other than the one written.
enum Path {
    Segments(Vec<String>),
    Dotted,
}

/// **The path `arg` names, if it names one**: `None` for an argument that is no endpoint (an
/// option, a field's `key=value`, a header). The query and fragment are cut, each segment is
/// percent-decoded and lower-cased, empty segments drop, a leading `api/v3` or `api/v4` drops,
/// and `repositories/<id>`, `organizations/<id>` read as `repos/<id>/<id>`, `orgs/<id>`.
fn path_of(arg: &str) -> Option<Path> {
    if arg.starts_with('-') {
        return None;
    }
    let path = arg.split(['?', '#']).next().unwrap_or_default();
    if path.contains('=') || path.chars().any(char::is_whitespace) {
        return None;
    }
    let mut segments = Vec::new();
    for raw in path.split(['/', '\\']).filter(|one| !one.is_empty()) {
        let one = percent_encoding::percent_decode_str(raw)
            .decode_utf8_lossy()
            .to_lowercase();
        if one == "." || one == ".." {
            return Some(Path::Dotted);
        }
        segments.push(one);
    }
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
    Some(Path::Segments(segments))
}

/// Whether `route`'s segments lead `segments`.
fn under(route: &str, segments: &[String]) -> bool {
    let route: Vec<&str> = route.split('/').collect();
    route.len() <= segments.len()
        && route
            .iter()
            .zip(segments)
            .all(|(want, one)| *want == "*" || *want == one)
}

/// **Why `forge`'s `api` run with `args` (its command first) never runs**, if it does not: an
/// endpoint with a dot segment; one under a route of the forge's list; GitLab's GraphQL, which
/// reaches its CI/CD variables' values and makes runner and agent tokens; or GitHub's, where the
/// query names a credential's field or is read from a file or standard input, so is never seen.
pub(super) fn refused(forge: Forge, args: &[String]) -> Option<String> {
    let routes = match forge {
        Forge::Gh => GH_ROUTES,
        Forge::Glab => GLAB_ROUTES,
    };
    let mut graphql = false;
    for arg in args.iter().skip(1) {
        match path_of(arg) {
            Some(Path::Dotted) => {
                return Some(format!(
                    "the endpoint {arg:?} has a `.` or `..` segment, which a server may read as \
                     another route"
                ));
            }
            Some(Path::Segments(segments)) => {
                if let Some(route) = routes.iter().find(|route| under(route, &segments)) {
                    return Some(format!(
                        "{route} shows or changes the forge's credentials or secrets, as a \
                         refused command does"
                    ));
                }
                graphql |= segments == ["graphql"];
            }
            None => {}
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
