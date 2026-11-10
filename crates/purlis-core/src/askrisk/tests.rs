// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A project at `/work/plane`, home `/home/me`, with `main` and `release/*` protected and
/// `example.com` marked as production.
fn facts() -> Facts {
    Facts::new(
        Path::new("/work/plane"),
        Some(PathBuf::from("/home/me")),
        ["main".to_owned(), "release/*".to_owned()],
        ["Example.COM".to_owned()],
    )
}

fn class(risk: Option<Risk>) -> Option<Class> {
    risk.map(|risk| risk.class)
}

#[test]
fn a_push_to_a_protected_branch_says_which_branch_and_asks_once_more() {
    let risk = facts().of_command("git push origin main").expect("risky");
    assert_eq!(risk.class, Class::ProtectedPush);
    assert_eq!(risk.why, "Pushes to main, a protected branch");
    assert_eq!(risk.confirm, "Allow a push to main?");
}

#[test]
fn a_push_is_read_through_its_refspec_options_and_wrappers() {
    let f = facts();
    for line in [
        "git push origin HEAD:main",
        "git push origin +feature:refs/heads/main",
        "git push --force-with-lease origin main",
        "git push -o ci.skip origin release/1.2",
        "git -C repos/app push origin main",
        "git push --delete origin main",
        "git push origin :main",
        "cd repos/app && git push origin main",
        "env GIT_TRACE=1 git push origin main",
        "sh -c 'git push origin main'",
        "git push --all origin",
    ] {
        assert_eq!(
            class(f.of_command(line)),
            Some(Class::ProtectedPush),
            "{line}"
        );
    }
}

#[test]
fn a_push_to_another_branch_or_to_no_named_branch_is_not_classed() {
    let f = facts();
    for line in [
        "git push origin feature/x",
        "git push origin HEAD",
        "git push",
        "git push origin",
        "git push origin refs/tags/main",
        "git push --repo main origin feature",
        "git log main",
        "echo git push origin main",
        "git push origin mainline",
        "git push origin release",
    ] {
        assert_eq!(class(f.of_command(line)), None, "{line}");
    }
}

#[test]
fn every_branch_pushed_is_protected_only_where_the_project_protects_one() {
    let none = Facts::new(Path::new("/work/plane"), None, Vec::new(), Vec::new());
    assert_eq!(none.of_command("git push --all origin"), None);
    assert_eq!(none.of_command("git push origin main"), None);
}

#[test]
fn a_host_that_looks_like_production_is_one_with_a_prod_label() {
    let f = facts();
    let risk = f.of_host("api.prod.acme.io:443").expect("risky");
    assert_eq!(risk.class, Class::ProductionHost);
    assert_eq!(
        risk.why,
        "Reaches api.prod.acme.io, a host that looks like production"
    );
    assert_eq!(risk.confirm, "Allow reaching api.prod.acme.io?");
    for host in ["prod-db.internal", "production.acme.io", "PROD.acme.io."] {
        assert_eq!(
            class(f.of_host(host)),
            Some(Class::ProductionHost),
            "{host}"
        );
    }
    for host in [
        "product.acme.io",
        "prodigy.net",
        "staging.acme.io",
        "",
        "[::1]:8080",
    ] {
        assert_eq!(class(f.of_host(host)), None, "{host}");
    }
}

#[test]
fn a_host_the_project_marks_as_production_and_its_subdomains_are_production() {
    let f = facts();
    assert_eq!(
        f.of_host("api.example.com").map(|risk| risk.why).as_deref(),
        Some("Reaches api.example.com, a production host of this project")
    );
    assert_eq!(class(f.of_host("example.com")), Some(Class::ProductionHost));
    assert_eq!(class(f.of_host("notexample.com")), None);
}

#[test]
fn a_command_or_a_tool_that_reaches_a_production_url_is_classed() {
    let f = facts();
    assert_eq!(
        class(f.of_command("curl -s https://api.prod.acme.io:8443/v1/deploy")),
        Some(Class::ProductionHost)
    );
    assert_eq!(class(f.of_command("curl https://staging.acme.io")), None);
    assert_eq!(
        class(f.of_tool(r#"{"url":"https://shop.example.com/admin","prompt":"x"}"#)),
        Some(Class::ProductionHost)
    );
    assert_eq!(class(f.of_tool(r#"{"url":"https://docs.rs"}"#)), None);
    assert_eq!(class(f.of_tool("not json")), None);
}

#[test]
fn a_write_outside_the_project_says_where() {
    let f = facts();
    let risk = f.of_write("/etc/hosts").expect("risky");
    assert_eq!(risk.class, Class::OutsideProject);
    assert_eq!(risk.why, "Writes outside the project, in /etc/hosts");
    assert_eq!(risk.confirm, "Allow a write outside the project?");
    for path in ["~/.ssh/config", "/work/plane/../other", "/work/planet/x"] {
        assert_eq!(
            class(f.of_write(path)),
            Some(Class::OutsideProject),
            "{path}"
        );
    }
}

#[test]
fn a_write_in_the_project_or_on_a_relative_path_is_not_classed() {
    let f = facts();
    for path in [
        "/work/plane",
        "/work/plane/workspaces/a/repos/app/src/main.rs",
        "/work/plane/./x/../y",
        "src/main.rs",
    ] {
        assert_eq!(class(f.of_write(path)), None, "{path}");
    }
    // With no home known, `~` is not guessed at.
    let homeless = Facts::new(Path::new("/work/plane"), None, Vec::new(), Vec::new());
    assert_eq!(homeless.of_write("~/x"), None);
}

#[test]
fn the_facts_are_read_from_the_inventory_and_the_risk_table() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("inventory")).expect("inventory");
    std::fs::write(
        root.join("inventory/repos.json"),
        r#"{"repos":[{"name":"app","default_branch":"trunk"},{"name":"web"}]}"#,
    )
    .expect("repos");
    std::fs::write(
        root.join(crate::profiles::COMMITTED_FILE),
        "[risk]\nprotected_branches = [\"stable\", 3]\nproduction_hosts = [\"shop.test\"]\n",
    )
    .expect("manifest");
    let f = Facts::read(root);
    assert_eq!(
        class(f.of_command("git push origin trunk")),
        Some(Class::ProtectedPush)
    );
    assert_eq!(
        class(f.of_command("git push origin stable")),
        Some(Class::ProtectedPush)
    );
    assert_eq!(class(f.of_command("git push origin main")), None);
    assert_eq!(
        class(f.of_host("cdn.shop.test")),
        Some(Class::ProductionHost)
    );
    let inside = root.join("workspaces/x").display().to_string();
    assert_eq!(f.of_write(&inside), None);
}

#[test]
fn a_project_with_no_facts_on_disk_protects_nothing_but_still_reads_hosts_and_paths() {
    let dir = tempfile::tempdir().expect("dir");
    let f = Facts::read(dir.path());
    assert_eq!(f.of_command("git push origin main"), None);
    assert_eq!(
        class(f.of_host("prod.acme.io")),
        Some(Class::ProductionHost)
    );
    assert_eq!(class(f.of_write("/etc/x")), Some(Class::OutsideProject));
}

#[test]
fn a_pattern_matches_a_whole_name() {
    assert!(matches("release/*", "release/1.2"));
    assert!(matches("*-stable", "v1-stable"));
    assert!(matches("a*b*c", "axxbyyc"));
    assert!(!matches("release/*", "release"));
    assert!(!matches("main", "main2"));
    assert!(!matches("a*b*c", "acb"));
}
