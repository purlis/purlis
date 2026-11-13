// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// The key's parts are checked as they are made and as they are read off the wire: a command,
/// a URL or a short commit is never a merge's key.
#[test]
fn a_merges_key_is_checked_as_it_is_made_and_read() {
    for repo in ["o/r", "group/sub/project", "my.org/re_po-1"] {
        assert!(Repo::try_from(repo).is_ok(), "{repo}");
    }
    for repo in [
        "r",
        "o/r; rm -rf ~",
        "o/../r",
        "https://github.com/o/r",
        "o//r",
        "o/r/",
    ] {
        assert!(Repo::try_from(repo).is_err(), "{repo}");
    }
    assert!(HeadSha::try_from("0123456789abcdef0123456789abcdef01234567").is_ok());
    for head in [
        "0123456",
        "0123456789ABCDEF0123456789ABCDEF01234567",
        "HEAD",
    ] {
        assert!(HeadSha::try_from(head).is_err(), "{head}");
    }
    let wire =
        r#"{"repo":"o/r; id","number":12,"head":"0123456789abcdef0123456789abcdef01234567"}"#;
    assert!(serde_json::from_str::<MergeKey>(wire).is_err());
}

/// A value flag's value is never read as the request's number, and a repo flag that names no
/// repo leaves the request unread.
#[test]
fn a_merges_request_is_read_past_its_flags() {
    let read = |cmd: &str| held(cmd);
    assert_eq!(
        read("gh pr merge --subject 7 12 --repo=o/r"),
        Some(Held::Request(Request {
            repo: Some(Repo::try_from("o/r").unwrap()),
            number: 12
        }))
    );
    assert_eq!(read("gh pr merge -R 'o/r; id' 12"), Some(Held::Unreadable));
    assert_eq!(read("gh pr merge feature-branch"), Some(Held::Unreadable));
}

/// Q239 (S3): the hook's refusal is a hold only where the app confirmed it holds the ask, and
/// the chat is told the person is asked only then; every other answer is a block, and says
/// nobody was asked.
#[test]
fn a_held_merge_is_a_hold_only_where_the_app_confirmed_its_ask() {
    let request = Held::Request(Request {
        repo: None,
        number: 12,
    });
    let held = AppSaid::Held("The person is asked in Asks to merge request 12.");
    assert_eq!(reason(&request, &held), REASON);
    assert!(refusal(&request, &held).contains("The person is asked"));
    for app in [
        AppSaid::Refused("chat 4 is not one this app has open."),
        AppSaid::Unheard,
    ] {
        assert_eq!(reason(&request, &app), REASON_NOT_ASKED, "{app:?}");
        let said = refusal(&request, &app);
        assert!(
            said.contains("nobody was asked") || said.contains("Nobody was asked"),
            "{said}"
        );
        assert!(!said.contains("The person is asked"), "{said}");
    }
    assert_eq!(reason(&Held::Unreadable, &held), REASON_UNREADABLE);
}

/// Q239 (S8): a repo is the project's only where its inventory, read as `inventory::load` reads
/// it, lists it on the same forge, host and path; a link that takes the inventory out of the
/// project lists nothing.
#[test]
fn a_repo_is_the_projects_only_on_its_listed_host_and_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("inventory")).unwrap();
    let row = |web: &str| serde_json::json!({"path_with_namespace": "o/r", "forge": "github", "web_url": web});
    let write = |rows: Vec<serde_json::Value>| {
        std::fs::write(
            crate::inventory::path(root),
            serde_json::json!({"repos": rows}).to_string(),
        )
        .unwrap();
    };
    let github = crate::forge::Forge::default_of(crate::forge::Kind::GitHub);
    write(vec![row("https://github.com/o/r")]);
    assert!(listed_in(root, &github, "o/r"));
    assert!(!listed_in(root, &github, "o/other"));
    let elsewhere = crate::forge::Forge {
        kind: crate::forge::Kind::GitHub,
        host: "git.example.com".to_owned(),
    };
    assert!(
        !listed_in(root, &elsewhere, "o/r"),
        "same path, another host"
    );
    write(vec![row("")]);
    assert!(
        !listed_in(root, &github, "o/r"),
        "a row with no address names no host"
    );

    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("repos.json"),
        serde_json::json!({"repos": [row("https://github.com/o/r")]}).to_string(),
    )
    .unwrap();
    std::fs::remove_dir_all(root.join("inventory")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("inventory")).unwrap();
    assert!(
        !listed_in(root, &github, "o/r"),
        "a linked inventory lists nothing"
    );
}

/// Q239: `purlis change land` is held in a chat the app started, and its refusal says what it
/// is, a landing the person does now, not a `gh pr merge` it never ran.
#[test]
fn a_change_land_is_held_and_told_it_is_the_persons_now() {
    for cmd in [
        "purlis change land api-2",
        "charter change land api-2 --repo api",
    ] {
        assert_eq!(held(cmd), Some(Held::Land), "{cmd}");
        let said = refusal(&Held::Land, &AppSaid::Unheard);
        assert!(said.contains("`purlis change land`"), "{said}");
        assert!(said.contains("nobody was asked"), "{said}");
        assert!(!said.contains("cannot tell which request"), "{said}");
        assert_eq!(reason(&Held::Land, &AppSaid::Unheard), REASON_UNREADABLE);
    }
}

/// The head a merge ask's forge read is of, in [`the_ruleset_check_uses_the_apps_forge_login_not_the_chats_token`].
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// A transport that answers `exchanges`, as the forge contract's recordings do.
fn recorded(exchanges: serde_json::Value) -> std::sync::Arc<crate::forge::recorded::Recorded> {
    let text = serde_json::json!({"source": "GitHub REST API version 2022-11-28: pulls, checks, \
                                             commits' status, and rules for a branch",
                                  "exchanges": exchanges});
    std::sync::Arc::new(crate::forge::recorded::Recorded::parse(&text.to_string()).unwrap())
}

/// A chat's token: it answers nothing, and keeps every request it was sent.
#[derive(Debug, Default)]
struct ChatsToken {
    asked: Mutex<Vec<crate::forge::transport::Call>>,
}

impl crate::forge::transport::Transport for ChatsToken {
    fn send(
        &self,
        _forge: &crate::forge::Forge,
        call: &crate::forge::transport::Call,
    ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
        self.asked.lock().unwrap().push(call.clone());
        Err(crate::forge::transport::NoAnswer::Refused(
            "a chat's token".to_owned(),
        ))
    }

    fn check_auth(&self, _forge: &crate::forge::Forge) -> Result<(), crate::forge::ForgeError> {
        Ok(())
    }
}

/// One transport for a person's call, the app's own login, and another for a chat's, its token.
struct Logins {
    app: std::sync::Arc<crate::forge::recorded::Recorded>,
    chat: std::sync::Arc<ChatsToken>,
}

impl crate::forge::backend::Transports for Logins {
    fn for_caller(
        &self,
        caller: &crate::forge::backend::Caller,
    ) -> Result<std::sync::Arc<dyn crate::forge::transport::Transport>, crate::forge::ForgeError>
    {
        Ok(if caller.is_a_chat() {
            self.chat.clone()
        } else {
            self.app.clone()
        })
    }
}

/// R-146: a merge ask's forge read asks whether a ruleset protects the request's base with the
/// app's own login, and nothing of it goes through a chat's.
#[test]
fn the_ruleset_check_uses_the_apps_forge_login_not_the_chats_token() {
    use serde_json::json;
    let get = |path: &str, out: serde_json::Value| {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": {"code": 0, "out": out.to_string(), "err": ""}})
    };
    let pulls = json!({"number": 12, "title": "Add the parser", "user": {"login": "octo"},
                       "head": {"sha": HEAD},
                       "base": {"ref": "main", "repo": {"allow_merge_commit": true}}});
    let runs = json!({"total_count": 1, "check_runs": [{"id": 4, "status": "completed",
                                                        "conclusion": "success"}]});
    let status = json!({"state": "success", "total_count": 0, "statuses": []});
    let rules = json!([{"type": "pull_request", "ruleset_source_type": "Repository",
                        "ruleset_source": "o/r", "ruleset_id": 42}]);
    let logins = std::sync::Arc::new(Logins {
        app: recorded(json!([
            get("repos/o/r/pulls/12", pulls),
            get(
                &format!("repos/o/r/commits/{HEAD}/check-runs?per_page=100"),
                runs
            ),
            get(
                &format!("repos/o/r/commits/{HEAD}/status?per_page=100"),
                status
            ),
            get("repos/o/r/rules/branches/main", rules),
        ])),
        chat: std::sync::Arc::default(),
    });
    let backend =
        crate::forge::Forge::default_of(crate::forge::Kind::GitHub).backend_for(logins.clone());

    let read = read(
        std::sync::Arc::from(backend),
        "o/r",
        12,
        THE_FORGE_IS_READ_WITHIN,
        40,
    )
    .expect("the forge was read");

    assert_eq!(read.facts.ruleset, Ruleset::Protects);
    assert!(
        logins.app.unspent().is_empty(),
        "the app's login was asked everything, the ruleset too: {:?}",
        logins.app.unspent()
    );
    let asked = logins.chat.asked.lock().unwrap();
    assert!(asked.is_empty(), "a chat's token was used: {asked:?}");
}

/// The GitHub exchanges of request 12 of `o/r` at [`HEAD`], into `main`: the request, its
/// checks (one passed run, no statuses), and the `rules` reply for `main`.
fn request_12(rules: serde_json::Value) -> serde_json::Value {
    use serde_json::json;
    let get = |path: &str, out: serde_json::Value| {
        json!({"call": {"endpoint": {"rest": {"method": null, "path": path}}, "fields": []},
               "reply": {"code": 0, "out": out.to_string(), "err": ""}})
    };
    let pulls = json!({"number": 12, "title": "Add the parser", "user": {"login": "octo"},
                       "head": {"sha": HEAD},
                       "base": {"ref": "main", "repo": {"allow_merge_commit": true}}});
    let runs = json!({"total_count": 1, "check_runs": [{"id": 4, "status": "completed",
                                                        "conclusion": "success"}]});
    let status = json!({"state": "success", "total_count": 0, "statuses": []});
    let mut rules_exchange = get("repos/o/r/rules/branches/main", json!([]));
    rules_exchange["reply"] = rules;
    json!([
        get("repos/o/r/pulls/12", pulls),
        get(
            &format!("repos/o/r/commits/{HEAD}/check-runs?per_page=100"),
            runs
        ),
        get(
            &format!("repos/o/r/commits/{HEAD}/status?per_page=100"),
            status
        ),
        rules_exchange,
    ])
}

/// The app's login over `recorded`, where a call whose path holds `slow` is answered only
/// after `late`.
struct Slow {
    recorded: std::sync::Arc<crate::forge::recorded::Recorded>,
    slow: &'static str,
    late: std::time::Duration,
}

impl crate::forge::transport::Transport for Slow {
    fn send(
        &self,
        forge: &crate::forge::Forge,
        call: &crate::forge::transport::Call,
    ) -> Result<crate::forge::transport::Reply, crate::forge::transport::NoAnswer> {
        if format!("{call:?}").contains(self.slow) {
            std::thread::sleep(self.late);
        }
        self.recorded.send(forge, call)
    }

    fn check_auth(&self, _forge: &crate::forge::Forge) -> Result<(), crate::forge::ForgeError> {
        Ok(())
    }
}

/// A GitHub backend over `transport`, as the app's login.
fn github_over(
    transport: std::sync::Arc<dyn crate::forge::transport::Transport>,
) -> Box<dyn crate::forge::backend::ForgeBackend> {
    crate::forge::Forge::default_of(crate::forge::Kind::GitHub).backend_over(transport)
}

/// Q248: the head alone gates a merge ask. Checks the forge is slow to give are unknown, and
/// the ask is still raised, at the head and with the ruleset the forge did give, in time.
#[test]
fn a_slow_checks_read_is_unknown_and_the_ask_is_still_raised() {
    let rules = serde_json::json!({"code": 0, "err": "", "out": serde_json::json!(
        [{"type": "pull_request", "ruleset_source_type": "Repository", "ruleset_source": "o/r",
          "ruleset_id": 42}]).to_string()});
    let backend = github_over(std::sync::Arc::new(Slow {
        recorded: recorded(request_12(rules)),
        slow: "check-runs",
        late: THE_FORGE_IS_READ_WITHIN * 3,
    }));
    let started = std::time::Instant::now();

    let read = read(
        std::sync::Arc::from(backend),
        "o/r",
        12,
        THE_FORGE_IS_READ_WITHIN,
        40,
    )
    .expect("the head was read in time");

    assert!(
        started.elapsed() < THE_FORGE_IS_READ_WITHIN * 2,
        "the read waited for the late checks: {:?}",
        started.elapsed()
    );
    assert_eq!(read.facts.checks.ci, crate::forge::checks::Ci::Unknown);
    assert_eq!(read.facts.ruleset, Ruleset::Protects);
    let holds = Holds::default();
    let key = MergeKey {
        repo: Repo::try_from("o/r").unwrap(),
        number: 12,
        head: HeadSha::try_from(read.at.sha.as_str()).unwrap(),
        base: Base::try_from(read.at.base.as_str()).unwrap(),
    };
    let resolve = |_: &Request| Ok((key.clone(), read.facts.clone()));
    let request = Request {
        repo: None,
        number: 12,
    };
    let (_, raised) = asked(&holds, 4, &request, &resolve, 40).expect("asked");
    assert_eq!(raised, Raised::New);
    assert_eq!(
        holds.every()[0].facts.checks.ci,
        crate::forge::checks::Ci::Unknown
    );
}

/// A ruleset read the forge fails is unknown, in its words, never none.
#[test]
fn a_failed_ruleset_read_is_unknown_never_none() {
    let refused = serde_json::json!({"code": 1, "status": 500,
        "out": "{\"message\": \"Server Error\"}", "err": "gh: Server Error (HTTP 500)"});
    let backend = github_over(recorded(request_12(refused)));

    let read = read(
        std::sync::Arc::from(backend),
        "o/r",
        12,
        THE_FORGE_IS_READ_WITHIN,
        40,
    )
    .expect("the head was read");

    let Ruleset::Unknown(why) = &read.facts.ruleset else {
        panic!("{:?}", read.facts.ruleset);
    };
    assert!(why.contains("Server Error"), "{why}");
}
