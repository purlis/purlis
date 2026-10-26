// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The built-in lints, each fired on a fixture skill (SK-8, #2771).

use super::*;
use crate::change::Problem;
use crate::compat::Field;

/// A machine with `git`, `sh`, `bash` and `python3`, and nothing else.
fn has(tool: &str) -> bool {
    ["git", "sh", "bash", "python3", "/bin/sh"].contains(&tool)
}

fn lints<'a>(
    plane: &'a std::collections::HashSet<String>,
    built_in: &'a std::collections::HashSet<String>,
) -> BuiltInLints<'a> {
    BuiltInLints {
        namespace: "skills",
        plane,
        built_in,
        known_as: built_in,
        has: &has,
    }
}

fn found(text: &str, others: &[(String, String)], handed: bool) -> Vec<Finding> {
    let origin = if handed {
        crate::Origin::Project
    } else {
        crate::Origin::Repo
    };
    let plane = ["deploy".to_owned(), "review".to_owned()].into();
    let built_in = ["secrets".to_owned()].into();
    lints(&plane, &built_in).findings(&SkillFiles {
        name: "deploy",
        text,
        others,
        origin,
        plugin: None,
    })
}

fn says(findings: &[Finding]) -> Vec<String> {
    findings.iter().map(ToString::to_string).collect()
}

fn skill(more: &str, body: &str) -> String {
    format!(
        "---\nname: deploy\ndescription: Deploys the app to staging, then checks it.\n{more}---\n\n{body}\n"
    )
}

#[test]
fn a_whole_skill_that_calls_what_this_machine_has_is_clean() {
    let text = skill(
        "",
        "Run !`git status` first, then use skills:review and purlis:secrets.",
    );
    assert_eq!(found(&text, &[], true), Vec::<Finding>::new());
}

#[test]
fn a_frontmatter_problem_blocks_and_no_frontmatter_is_all_that_is_said() {
    let findings = found("no frontmatter, and !`jq .`", &[], true);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Blocks);
    let findings = found("---\nname: deploy\n---\n", &[], true);
    assert_eq!(findings[0].severity, Severity::Blocks);
    assert_eq!(
        findings[0].lint,
        Lint::Frontmatter(Problem::NoDescription),
        "{findings:?}"
    );
}

#[test]
fn a_description_too_short_to_say_when_to_use_it_warns() {
    let findings = found("---\nname: deploy\ndescription: Deploys.\n---\n", &[], true);
    assert_eq!(
        says(&findings),
        [
            "its description: it is 8 characters, and a chat chooses a skill by what its \
             description says, so say what it does and when to use it"
        ]
    );
    assert_eq!(findings[0].severity, Severity::Warns);
}

#[test]
fn a_key_nothing_reads_warns() {
    let findings = found(&skill("owner: ops\nlicense: MIT\n", ""), &[], true);
    assert_eq!(
        says(&findings),
        ["its frontmatter's owner: is read by nothing that runs a skill"]
    );
}

#[test]
fn what_would_widen_the_chat_is_said_left_out_of_a_skill_purlis_hands_and_widening_otherwise() {
    let text = skill(
        "allowed-tools: Bash\nhooks:\n  Stop:\n    - command: x\n",
        "",
    );
    let handed = says(&found(&text, &[], true));
    assert_eq!(handed.len(), 2, "{handed:?}");
    assert!(
        handed[0].starts_with("its allowed-tools: would widen the chat, so purlis leaves it out")
    );
    assert!(handed[1].starts_with("its hooks: would run commands in the chat"));
    let outside = found(&text, &[], false);
    assert_eq!(
        outside,
        [Finding {
            severity: Severity::Warns,
            lint: Lint::Widens(vec![Field::AllowedTools, Field::Hooks]),
        }]
    );
    assert_eq!(
        outside[0].shown().as_deref(),
        Some(
            "Widens the chat: the tools it pre-approves and the commands it adds to the chat's \
             events apply while it runs, and purlis cannot leave them out of a skill it does \
             not hand over."
        )
    );
}

#[test]
fn a_reference_to_a_skill_there_is_none_of_warns_once() {
    let text = skill(
        "",
        "Use skills:reveiw, then skills:reveiw again, and purlis:secret. Not myskills:x or \
         https://skills:8080.",
    );
    assert_eq!(
        says(&found(&text, &[], true)),
        [
            "it names skills:reveiw, and this project has no skill reveiw: a chat told to use \
             it finds none",
            "it names purlis:secret, and purlis has no skill secret: a chat told to use it \
             finds none",
        ]
    );
}

#[test]
fn a_tool_an_injected_command_calls_that_this_machine_lacks_warns() {
    let text = skill(
        "",
        "Context: !`git log -3 | jq -r . && FOO=1 yq e .` and !`${CLAUDE_SKILL_DIR}/x.sh`.",
    );
    assert_eq!(
        says(&found(&text, &[], true)),
        [
            "its ! command calls jq, which this machine does not have",
            "its ! command calls yq, which this machine does not have",
        ]
    );
}

#[test]
fn a_script_warns_of_its_interpreter_and_each_program_a_shell_script_runs() {
    let shell = "#!/usr/bin/env bash\nset -euo pipefail\n# kubectl in a comment\nusage() {\n  echo hi\n}\ncase \"$1\" in\n  start) helm install x ;;\n  *) usage ;;\nesac\nif grep -q x f; then\n  gh pr view \\\n    --json x\nfi\ncat <<EOF\nterraform here-doc\nEOF\nout=$(jq . f 2>&1)\n./own.sh\n";
    let python = "#!/usr/bin/env python3\nimport os\n";
    let node = "#!/usr/bin/env node\nconsole.log(1)\n";
    let others = vec![
        ("scripts/run.sh".to_owned(), shell.to_owned()),
        ("scripts/check.py".to_owned(), python.to_owned()),
        ("scripts/x.js".to_owned(), node.to_owned()),
        (
            "reference.md".to_owned(),
            "Run `kubectl` by hand.".to_owned(),
        ),
    ];
    assert_eq!(
        says(&found(&skill("", ""), &others, true)),
        [
            "scripts/run.sh calls helm, which this machine does not have",
            "scripts/run.sh calls grep, which this machine does not have",
            "scripts/run.sh calls gh, which this machine does not have",
            "scripts/run.sh calls cat, which this machine does not have",
            "scripts/run.sh calls jq, which this machine does not have",
            "scripts/x.js calls node, which this machine does not have",
        ]
    );
}

#[test]
fn a_frontmatter_purlis_cannot_read_as_yaml_blocks_a_skill_it_hands_and_warns_of_one_it_does_not() {
    let text = "---\nname: deploy\ndescription: Deploys the app to staging, then checks it.\nallowed_tools: Bash\n---\n";
    let handed = found(text, &[], true);
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert_eq!(handed[0].severity, Severity::Blocks);
    assert!(
        handed[0]
            .to_string()
            .starts_with("purlis hands it to no chat")
    );
    let outside = found(text, &[], false);
    assert_eq!(outside[0].severity, Severity::Warns);
    assert!(
        outside[0]
            .shown()
            .unwrap()
            .starts_with("It may widen the chat")
    );
}

#[test]
fn only_the_editor_names_a_key() {
    let findings = found(&skill("owner: ops\nallowed-tools: Bash\n", ""), &[], true);
    assert_eq!(findings.len(), 2);
    for finding in &findings {
        assert_eq!(finding.shown(), None, "{finding}");
    }
}

#[test]
fn a_skill_naming_itself_names_one_there_is_though_the_plane_has_no_skill_of_its_name() {
    // SK-44 (#3133): the plane's names are worked out once per list, without each skill's own
    // name in them; a repo skill telling the chat to use itself again is no broken reference.
    let text = "---\nname: tidy\ndescription: Tidies the repo, then checks it again.\n---\n\n\
                When done, use skills:tidy again and skills:nowhere.\n";
    let none = std::collections::HashSet::new();
    let findings = lints(&none, &none).findings(&SkillFiles {
        name: "tidy",
        text,
        others: &[],
        origin: crate::Origin::Repo,
        plugin: None,
    });
    assert_eq!(
        says(&findings),
        [
            "it names skills:nowhere, and this project has no skill nowhere: a chat told to use it \
          finds none"
        ]
    );
}

#[test]
fn a_repo_or_harness_skill_named_like_a_built_in_one_warns_and_a_skill_purlis_hands_does_not() {
    // SK-37 (#3027): a program reads a clone's `secrets` beside purlis's own, and the chat may
    // use either. purlis hands a plane skill of that name to no chat, which its own row says.
    let text = "---\nname: secrets\ndescription: Reads the repo's secrets from its vault.\n---\n";
    let plane = std::collections::HashSet::new();
    let built_in = ["secrets".to_owned()].into();
    let said = |origin, plugin| {
        let findings = lints(&plane, &built_in).findings(&SkillFiles {
            name: "secrets",
            text,
            others: &[],
            origin,
            plugin,
        });
        findings
            .iter()
            .map(|one| (one.severity, one.to_string(), one.shown()))
            .collect::<Vec<_>>()
    };

    for origin in [crate::Origin::Repo, crate::Origin::Harness] {
        let line = "it has the name of one of purlis's built-in skills, so a chat gets purlis's \
                    too and may use either: rename it";
        assert_eq!(
            said(origin, None),
            [(Severity::Warns, line.to_owned(), Some(line.to_owned()))],
            "{origin:?}"
        );
    }
    for origin in [crate::Origin::BuiltIn, crate::Origin::Project] {
        assert_eq!(said(origin, None), [], "{origin:?}");
    }
    assert_eq!(
        said(crate::Origin::Harness, Some("figma")),
        [],
        "figma:secrets is known by its plugin's name, never as purlis's secrets"
    );
}

#[test]
fn on_a_route_that_names_built_in_skills_their_own_only_that_name_warns() {
    // SK-37 (#3027): opencode knows purlis's `secrets` as `purlis-secrets`, and its index skill
    // as `purlis-skill-index`, so a repo's bare `secrets` is another skill there.
    let plane = std::collections::HashSet::new();
    let built_in: std::collections::HashSet<String> = ["secrets".to_owned()].into();
    let warned = |route: crate::Route, name: &str| {
        let known_as = route.built_in_names(built_in.iter().map(String::as_str));
        let text = format!(
            "---\nname: {name}\ndescription: Reads the repo's secrets from a vault.\n---\n"
        );
        BuiltInLints {
            namespace: "skills",
            plane: &plane,
            built_in: &built_in,
            known_as: &known_as,
            has: &has,
        }
        .findings(&SkillFiles {
            name,
            text: &text,
            others: &[],
            origin: crate::Origin::Repo,
            plugin: None,
        })
        .iter()
        .any(|one| one.lint == Lint::BuiltInName)
    };

    assert!(warned(crate::Route::Config, "purlis-secrets"));
    assert!(warned(crate::Route::Config, "purlis-skill-index"));
    assert!(!warned(crate::Route::Config, "secrets"));
    for route in [crate::Route::Plugin, crate::Route::Briefing] {
        assert!(warned(route, "secrets"), "{route:?}");
        assert!(!warned(route, "purlis-secrets"), "{route:?}");
    }
}
