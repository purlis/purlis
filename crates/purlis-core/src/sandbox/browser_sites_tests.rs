// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The project's browser sites (#2443): `[sandbox] browser-sites` in the committed file, in
//! force on a machine once the person there was shown each one, for browsers alone.

use super::*;

fn a_project(manifest: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join(crate::plane::MANIFEST), manifest).expect("the manifest");
    dir
}

fn manifest(root: &Path) -> String {
    std::fs::read_to_string(root.join(crate::plane::MANIFEST)).expect("the manifest")
}

const SITES: &str = "[sandbox]\nmode = \"on\"\nhosts = [\"api.example.com\"]\nbrowser-sites = [\"news.example.org:443\", \"docs.example.com\"]\n";

fn written(hosts: &[Host]) -> Vec<String> {
    hosts.iter().map(ToString::to_string).collect()
}

fn none() -> super::super::policy::Locks {
    super::super::policy::Locks::default()
}

#[test]
fn a_teammates_browser_site_reaches_no_browser_here_until_it_was_shown() {
    let project = a_project(SITES);

    assert_eq!(in_force(project.path(), &none()), Vec::<Host>::new());
    let told = changed(project.path(), &none()).expect("told");
    assert_eq!(told.added, ["news.example.org:443", "docs.example.com"]);
    assert_eq!(told.removed, Vec::<String>::new());

    acknowledge(project.path(), &told.added).expect("recorded");

    assert_eq!(
        written(&in_force(project.path(), &none())),
        ["news.example.org:443", "docs.example.com"]
    );
    assert_eq!(changed(project.path(), &none()), None, "told once");
}

#[test]
fn each_site_is_shown_on_its_own_so_one_added_later_is_told_alone() {
    let project = a_project(SITES);
    acknowledge(project.path(), &["news.example.org:443".to_owned()]).expect("recorded");

    assert_eq!(
        written(&in_force(project.path(), &none())),
        ["news.example.org:443"],
        "the one not shown waits"
    );
    assert_eq!(
        changed(project.path(), &none()).expect("told").added,
        ["docs.example.com"]
    );
}

#[test]
fn a_site_taken_out_is_told_and_forgotten_so_putting_it_back_is_told_again() {
    let project = a_project(SITES);
    let told = changed(project.path(), &none()).expect("told");
    acknowledge(project.path(), &told.added).expect("recorded");
    std::fs::write(
        project.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\"]\n",
    )
    .expect("written");

    let told = changed(project.path(), &none()).expect("told");
    assert_eq!(told.added, Vec::<String>::new());
    assert_eq!(told.removed, ["docs.example.com"]);
    acknowledge(project.path(), &told.added).expect("recorded");
    assert_eq!(changed(project.path(), &none()), None);

    std::fs::write(project.path().join(crate::plane::MANIFEST), SITES).expect("written");
    assert_eq!(
        changed(project.path(), &none()).expect("told").added,
        ["docs.example.com"]
    );
    assert_eq!(
        written(&in_force(project.path(), &none())),
        ["news.example.org:443"]
    );
}

#[test]
fn a_site_shown_is_recorded_only_while_the_project_still_lists_it() {
    let project = a_project(SITES);
    acknowledge(project.path(), &["elsewhere.example:443".to_owned()]).expect("recorded");
    std::fs::write(
        project.path().join(crate::plane::MANIFEST),
        "[sandbox]\nmode = \"on\"\nbrowser-sites = [\"elsewhere.example:443\"]\n",
    )
    .expect("written");
    assert_eq!(in_force(project.path(), &none()), Vec::<Host>::new());
}

#[test]
fn a_browser_site_is_never_one_of_the_hosts_the_chats_commands_reach() {
    let plane = Plane::of(Some(SITES));
    let policy = plane.in_force(&none()).expect("on");
    assert_eq!(written(&policy.hosts), ["api.example.com"]);
    assert_eq!(plane.granted_hosts(&none()), ["api.example.com"]);
    assert_eq!(plane.said().refused, []);
}

#[test]
fn an_entry_that_is_no_host_is_refused_and_reaches_nothing() {
    let text =
        "[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\", \"localhost\"]\n";
    let project = a_project(text);
    let said = Plane::of(Some(text)).said();
    assert_eq!(said.refused.len(), 1, "{:?}", said.refused);
    assert!(
        said.refused[0].to_string().contains("browser-sites"),
        "{}",
        said.refused[0]
    );
    assert_eq!(
        changed(project.path(), &none()).expect("told").added,
        ["news.example.org:443"]
    );
}

#[test]
fn browser_sites_that_are_not_a_list_are_refused() {
    let said = Plane::of(Some(
        "[sandbox]\nmode = \"on\"\nbrowser-sites = \"news.example.org\"\n",
    ))
    .said();
    assert_eq!(said.refused, [super::super::Refusal::BrowserSitesNotAList]);
}

#[test]
fn a_project_without_the_sandbox_has_no_browser_site_to_tell() {
    let project = a_project("browser-sites = []\n[sandbox]\nbrowser-sites = [\"a.example:443\"]\n");
    assert_eq!(changed(project.path(), &none()), None);
    assert_eq!(in_force(project.path(), &none()), Vec::<Host>::new());
}

#[test]
fn allowing_one_for_everyone_writes_it_to_the_manifest_and_counts_as_seen_here() {
    let project = a_project("schema = 1\n# ours\n[sandbox]\nmode = \"on\"\n");
    let site = Host::parse("news.example.org:443").expect("a host");

    grant(project.path(), &site).expect("written");
    grant(project.path(), &site).expect("once");

    assert_eq!(
        manifest(project.path()),
        "schema = 1\n# ours\n[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\"]\n"
    );
    assert_eq!(
        written(&in_force(project.path(), &none())),
        ["news.example.org:443"]
    );
    assert_eq!(
        changed(project.path(), &none()),
        None,
        "your own is no news to you"
    );
}

#[test]
fn removing_one_takes_it_out_of_the_manifest_and_the_key_with_the_last() {
    let project =
        a_project("[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\"]\n");
    let site = Host::parse("news.example.org:443").expect("a host");

    assert!(revoke(project.path(), &site).expect("removed"));
    assert!(!revoke(project.path(), &site).expect("nothing more"));

    assert_eq!(manifest(project.path()), "[sandbox]\nmode = \"on\"\n");
    assert_eq!(in_force(project.path(), &none()), Vec::<Host>::new());
}

#[test]
fn where_policy_removes_everyone_in_the_project_no_browser_site_is_told_or_in_force() {
    let project = a_project(SITES);
    let locks = super::super::policy::Locks::parse(
        "{\"allow-scopes\": [\"chat\", \"you\"]}",
        Path::new("/etc/purlis/policy.json"),
    );
    acknowledge(project.path(), &["news.example.org:443".to_owned()]).expect("recorded");

    assert_eq!(changed(project.path(), &locks), None);
    assert_eq!(in_force(project.path(), &locks), Vec::<Host>::new());
}

#[test]
fn a_site_removed_here_and_put_back_waits_to_be_shown_again() {
    let project = a_project("schema = 1\n[sandbox]\nmode = \"on\"\n");
    let site = Host::parse("news.example.org:443").expect("a host");
    grant(project.path(), &site).expect("written");
    assert_eq!(
        written(&in_force(project.path(), &none())),
        ["news.example.org:443"]
    );

    assert!(revoke(project.path(), &site).expect("removed"));
    std::fs::write(
        project.path().join(crate::plane::MANIFEST),
        "schema = 1\n[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\"]\n",
    )
    .expect("put back");

    assert_eq!(in_force(project.path(), &none()), Vec::<Host>::new());
    assert_eq!(
        changed(project.path(), &none()).expect("told").added,
        ["news.example.org:443"]
    );
}
