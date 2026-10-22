// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The built-in lints, each fired on a fixture skill (SK-8, #2771).

use super::*;

/// A machine with `git`, `sh`, `bash` and `python3`, and nothing else.
fn has(tool: &str) -> bool {
    ["git", "sh", "bash", "python3", "/bin/sh"].contains(&tool)
}

fn lints<'a>(plane: &'a [String], built_in: &'a [String]) -> BuiltInLints<'a> {
    BuiltInLints {
        namespace: "skills",
        plane,
        built_in,
        has: &has,
    }
}

fn found(text: &str, others: &[(String, String)], handed: bool) -> Vec<Finding> {
    let plane = vec!["deploy".to_owned(), "review".to_owned()];
    let built_in = vec!["secrets".to_owned()];
    lints(&plane, &built_in).findings(&SkillFiles {
        name: "deploy",
        text,
        others,
        handed,
    })
}

fn says(findings: &[Finding]) -> Vec<String> {
    findings.iter().map(ToString::to_string).collect()
}

fn skill(more: &str, body: &str) -> String {
    format!("---\nname: deploy\ndescription: Deploys the app to staging, then checks it.\n{more}---\n\n{body}\n")
}

#[test]
fn a_whole_skill_that_calls_what_this_machine_has_is_clean() {
    let text = skill("", "Run !`git status` first, then use skills:review and purlis:secrets.");
    assert_eq!(found(&text, &[], true), Vec::<Finding>::new());
}

#[test]
fn a_frontmatter_problem_blocks_and_no_frontmatter_is_all_that_is_said() {
    let findings = found("no frontmatter, and !`jq .`", &[], true);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].level, Level::Blocks);
    let findings = found("---\nname: deploy\n---\n", &[], true);
    assert_eq!(findings[0].level, Level::Blocks);
    assert!(findings[0].says.contains("no description"), "{findings:?}");
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
    assert_eq!(findings[0].level, Level::Warns);
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
    let text = skill("allowed-tools: Bash\nhooks:\n  Stop:\n    - command: x\n", "");
    let handed = says(&found(&text, &[], true));
    assert_eq!(handed.len(), 2, "{handed:?}");
    assert!(handed[0].starts_with("its allowed-tools: would widen the chat, so purlis leaves it out"));
    assert!(handed[1].starts_with("its hooks: would run commands in the chat"));
    let outside = says(&found(&text, &[], false));
    assert_eq!(
        outside,
        [
            "its allowed-tools: widens the chat: the tools it pre-approves apply while it runs",
            "its hooks: widens the chat: the commands it adds to the chat's events apply while \
             it runs",
        ]
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
        ("reference.md".to_owned(), "Run `kubectl` by hand.".to_owned()),
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
