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
    let built_in = ["purlis-secrets".to_owned()].into();
    lints(&plane, &built_in).findings(&SkillFiles {
        name: "deploy",
        text,
        others,
        origin,
        plugin: None,
        listed: None,
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
        "Run !`git status` first, then use skills:review, purlis:purlis-secrets and its old \
         name purlis:secrets.",
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
            "it names purlis:secret, and purlis has no skill purlis-secret: a chat told to use \
             it finds none",
        ]
    );
}

/// #4510: a skill written for every harness names its siblings bare. The lint says so, once each,
/// as a hint, and never of the skill itself, a quoted word that names no skill, or a name it
/// also references.
#[test]
fn a_bare_quoted_skill_name_beside_the_word_skill_warns_as_a_hint() {
    let text = skill(
        "",
        "Use the Skill tool with \"review\", then `review` again. This skill, `deploy`, \
         ships. The skill \"roasting\" is not one.",
    );
    assert_eq!(
        says(&found(&text, &[], true)),
        [
            "it names \"review\" without skills:, so purlis only guesses it means the skill \
             review: write skills:review so purlis checks it and keeps it right when review is \
             renamed",
        ]
    );
    let both = skill("", "Use skills:review, the skill \"review\".");
    assert!(says(&found(&both, &[], true)).is_empty());
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
        listed: None,
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
fn a_repo_or_harness_skill_with_a_reserved_name_warns_and_a_skill_purlis_hands_does_not() {
    // SK-37 (#3027): a program reads a clone's `purlis-secrets` beside purlis's own, and the chat
    // may use either. purlis hands a plane skill of that name to no chat, which its own row says.
    let text =
        "---\nname: purlis-secrets\ndescription: Reads the repo's secrets from its vault.\n---\n";
    let plane = std::collections::HashSet::new();
    let built_in = ["purlis-secrets".to_owned()].into();
    let said = |origin, plugin| {
        let findings = lints(&plane, &built_in).findings(&SkillFiles {
            name: "purlis-secrets",
            text,
            others: &[],
            origin,
            plugin,
            listed: None,
        });
        findings
            .iter()
            .map(|one| (one.severity, one.to_string(), one.shown()))
            .collect::<Vec<_>>()
    };

    for origin in [crate::Origin::Repo, crate::Origin::Harness] {
        let line = "a name starting with purlis- is purlis's own, so a chat may get purlis's \
                    skill of this name too and use either: rename it";
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
        "figma:purlis-secrets is known by its plugin's name, never as purlis's"
    );
}

/// What the lints say of `text`, a project skill whose folder holds `listed`.
fn said_with_files(text: &str, listed: Option<&[String]>) -> Vec<Option<String>> {
    let plane = ["deploy".to_owned()].into();
    let built_in = std::collections::HashSet::new();
    lints(&plane, &built_in)
        .findings(&SkillFiles {
            name: "deploy",
            text,
            others: &[],
            origin: crate::Origin::Project,
            plugin: None,
            listed,
        })
        .iter()
        .map(Finding::shown)
        .collect()
}

#[test]
fn a_script_the_skill_names_and_its_folder_does_not_have_warns_once() {
    // #3780: the prototype's deploy, whose step 3 runs a script it never shipped.
    let text = skill(
        "",
        "1. Build it.\n2. Run `scripts/build.sh`.\n3. Tag it with ./scripts/tag.sh, then \
         scripts/tag.sh again.\n",
    );
    let listed = ["scripts/build.sh".to_owned(), "notes.md".to_owned()];
    assert_eq!(
        said_with_files(&text, Some(&listed)),
        [Some(
            "it names scripts/tag.sh, which its folder doesn't have: a chat told to run it \
             finds none"
                .to_owned()
        )]
    );
}

#[test]
fn a_skill_whose_folder_is_not_listed_or_that_names_a_folder_a_glob_or_another_path_is_clean() {
    let text = skill(
        "",
        "Run scripts/helpers, scripts/*.sh, scripts/build-${TARGET}.sh, other/scripts/x.sh \
         and https://example.com/scripts/y.sh. See scripts/.\n\
         Then scripts/déploy.sh and scripts/c++-build.sh.\n",
    );
    let listed = [
        "scripts/helpers/a.sh".to_owned(),
        "scripts/déploy.sh".to_owned(),
        "scripts/c++-build.sh".to_owned(),
    ];
    assert_eq!(
        said_with_files(&text, Some(&listed)),
        Vec::<Option<String>>::new()
    );
    // A text not yet saved has no folder to check against.
    let missing = skill("", "Run scripts/tag.sh.\n");
    assert_eq!(
        said_with_files(&missing, None),
        Vec::<Option<String>>::new()
    );
}

/// The programs a shell script's lint names as missing on a machine with none of them.
fn missing_from(script: &str) -> Vec<String> {
    let others = [("scripts/run.sh".to_owned(), script.to_owned())];
    says(&found(&skill("", ""), &others, true))
        .into_iter()
        .map(|line| {
            line.strip_prefix("scripts/run.sh calls ")
                .and_then(|rest| rest.strip_suffix(", which this machine does not have"))
                .unwrap_or(&line)
                .to_owned()
        })
        .collect()
}

#[test]
fn words_inside_a_quoted_jq_program_or_css_are_data_and_not_programs_a_script_calls() {
    let script = r#"#!/usr/bin/env bash
gh issue list --json number -q '.[] | [.number, (.labels|join(","))] | @tsv' |
while read -r n; do echo "$n"; done
jq -n -r --arg repo "$REPO" '
  ($ms | sort_by(.title) | map(
    "<tr><td>\(.title | @html)</td></tr>"
  ) | join("\n")) as $rows |
"<html><style>
body{font:15px/1.5 system-ui;color:var(--fg)}
h1{font-size:1.4rem;margin:0}h2{font-size:1.1rem}
</style>\($rows)</html>"' > "$OUT"
cat <<'EOF'
body{font:15px/1.5 system-ui;margin:0} a{color:inherit}
EOF
printf '%s\n' "a | b; c & d (e)"
"#;
    assert_eq!(missing_from(script), ["gh", "jq", "cat"]);
}

#[test]
fn a_program_a_substitution_runs_is_found_inside_double_quotes_a_default_and_an_unquoted_heredoc() {
    let script = r#"#!/usr/bin/env bash
title="$(curl -s "$URL" | yq '.title | @sh')"
echo "made by `whoami` on ${HOST:-$(hostname)}"
cat <<EOF
built $(date +%F) by $(id -un)
EOF
"#;
    assert_eq!(
        missing_from(script),
        ["curl", "yq", "whoami", "hostname", "cat", "date", "id"]
    );
}

#[test]
fn what_a_script_hands_a_shell_is_read_as_a_script_wherever_the_shell_is_called() {
    let script = r#"#!/usr/bin/env bash
bash -c 'kubectl get pods | awk "{print \$1}"'
sh -ec "helm list; terraform plan"
eval "make build"
eval "$(brew shellenv)"
find . -name '*.log' -exec sh -c 'gzip "$1"' _ {} \;
xargs -I{} bash -c 'rsync {} host:'
bash -s <<'EOF'
docker ps
EOF
cat <<'EOF' | sh
podman ps
EOF
"#;
    assert_eq!(
        missing_from(script),
        [
            "kubectl",
            "awk",
            "helm",
            "terraform",
            "make",
            "brew",
            "find",
            "gzip",
            "xargs",
            "rsync",
            "docker",
            "cat",
            "podman",
        ]
    );
}

#[test]
fn a_heredoc_for_another_program_and_a_here_string_are_data() {
    let script = r#"#!/usr/bin/env bash
ssh host <<EOF
remote-only-tool
EOF
bash ./job.sh <<'EOF'
input-not-a-command
EOF
read -r a <<< "$x"
jqq .
"#;
    assert_eq!(missing_from(script), ["ssh", "jqq"]);
}

#[test]
fn arithmetic_an_array_a_test_and_a_case_pattern_or_comment_run_nothing() {
    let script = r#"#!/usr/bin/env bash
(( count++ ))
arr=(one two three)
case "$1" in
  # stop) the old way
  start|go) helm install x ;;
  'stop') make stop ;;
  (*) usage ;;
esac
[[ $x =~ ^(a|b)$ && -n $y ]] && npm test
if [[ $x =~ ^(foo|bar)$ ]]; then pnpm i; fi
"#;
    assert_eq!(
        missing_from(script),
        ["helm", "make", "usage", "npm", "pnpm"]
    );
}
