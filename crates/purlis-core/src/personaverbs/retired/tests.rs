// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_persona_s_description_is_read_along_its_chain_and_agent_description_answers_first() {
    let plane = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let dir = plane.path().join("personas").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("persona.md"), text).unwrap();
    };
    write(
        "base",
        "---\nrole: Base\ndescription: Keeps the lights on\n---\n",
    );
    write("kid", "---\nextends: base\nrole: Kid\n---\n");
    write(
        "own",
        "---\nrole: Own\ndescription: x\nagent-description: Runs nights\n---\n",
    );
    write("bare", "---\nrole: Bare\n---\n");
    assert_eq!(
        description_of(plane.path(), "kid").as_deref(),
        Some("Keeps the lights on")
    );
    assert_eq!(
        description_of(plane.path(), "own").as_deref(),
        Some("Runs nights")
    );
    assert_eq!(description_of(plane.path(), "bare"), None);
    assert_eq!(description_of(plane.path(), "ghost"), None);
}

fn generated(marker: &str, name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: \"The Ops persona.\"\n---\n<!-- {marker} from \
         personas/{name}/persona.md — edit the persona, not this file. -->\n\nThis \
         sub-agent acts as the **{name}** persona — Ops — in an\nisolated context. Adopt \
         the charter below as your role.\n\n# Ops\n"
    )
}

#[test]
fn a_file_in_the_generators_shape_is_purlis_s_under_either_spelling_of_the_marker() {
    for marker in crate::names::SYNC_AGENTS_MARKER.spellings() {
        assert_eq!(
            authorship("ops", &generated(marker, "ops")),
            Authorship::Generated,
            "{marker}"
        );
    }
}

#[test]
fn a_file_without_the_marker_is_hand_written() {
    let own = "---\nname: runner\ndescription: mine\n---\n\nRun the suite.\n";
    assert_eq!(authorship("runner", own), Authorship::HandWritten);
}

#[test]
fn a_marker_that_is_not_where_the_generator_wrote_it_is_somebody_s_edit() {
    let marker = crate::names::SYNC_AGENTS_MARKER.write;
    // Quoted in a hand-written file's prose.
    let quoting = format!("---\nname: guard\n---\n\nNever edit a file that says `{marker}`.\n");
    assert_eq!(authorship("guard", &quoting), Authorship::Edited);
    // A generated file somebody wrote a line above the marker of.
    let above = generated(marker, "ops").replace("---\n<!--", "---\nMy own note.\n<!--");
    assert_eq!(authorship("ops", &above), Authorship::Edited);
    // A generated file copied to another name: it is not that name's sub-agent.
    assert_eq!(
        authorship("ops-copy", &generated(marker, "ops")),
        Authorship::Edited
    );
    // And one with the frontmatter taken off.
    let headless = generated(marker, "ops").replacen("---\n", "", 1);
    assert_eq!(authorship("ops", &headless), Authorship::Edited);
}

#[test]
fn every_retired_key_has_a_sentence_that_names_its_ticket_and_no_other_key_has_one() {
    for key in KEYS {
        let said = key_notice(key, "x").expect(key);
        assert!(said.starts_with(&format!("`{key}:` is no longer read: ")));
        assert!(
            said.contains("(#1460)") || said.contains("(#1453)"),
            "{said}"
        );
    }
    for kept in [
        "model",
        "color",
        "description",
        "agent-description",
        "role",
        "disallowed-tools",
    ] {
        assert_eq!(key_notice(kept, "x"), None, "{kept}");
    }
}

#[test]
fn a_tool_list_says_what_widened_now_that_nothing_reads_it() {
    // D-1451-19: a persona that was read-only by its allow-list is not any more.
    let read_only = key_notice("agent-tools", "Bash, Read, Grep, Glob").unwrap();
    assert!(
        read_only.contains(
            "As a sub-agent this persona could not edit files; as a chat it can, and it \
             has every other tool of its harness too."
        ),
        "{read_only}"
    );
    let writer = key_notice("agent-tools", "Read, Edit , Bash").unwrap();
    assert!(
        writer.contains(
            "As a sub-agent this persona had only the tools listed; as a chat it has \
             every tool of its harness."
        ),
        "{writer}"
    );
    assert!(!writer.contains("could not edit files"));
}

#[test]
fn a_model_that_names_no_profile_says_what_the_chat_runs_on_instead() {
    // D-1451-20: a persona that named `haiku` now runs on the asking chat's model.
    let offered = ["claude".to_owned(), "codex".to_owned()];
    let said = model_notice("haiku", false, &offered).unwrap();
    assert!(
        said.contains(
            "A chat as this persona runs on the asking chat's profile, with that \
             profile's model and its cost, not on `haiku`."
        ),
        "{said}"
    );
    assert_eq!(model_notice("codex", false, &offered), None);
    assert!(
        model_notice("codex", true, &offered)
            .unwrap()
            .ends_with("Delete the line")
    );
}

#[test]
fn the_refusal_names_the_dispatch_route_and_says_a_helper_still_runs() {
    let said = subagent_refusal("devops", false);
    assert!(said.contains("`purlis dispatch --to devops`"), "{said}");
    assert!(said.contains("A helper that is not named for a persona still runs"));
}

#[test]
fn a_draft_s_refusal_does_not_name_a_route_that_is_refused_too() {
    // F4: a dispatch to a draft is refused, so the sentence must not send the chat there.
    let said = subagent_refusal("intern", true);
    assert!(!said.contains("purlis dispatch --to"), "{said}");
    assert!(
        said.contains("still a draft, and a draft persona runs no chat"),
        "{said}"
    );
    assert!(said.contains("A helper that is not named for a persona still runs"));
}

#[test]
fn a_charter_that_still_teaches_the_old_route_is_found_by_its_words() {
    assert_eq!(
        old_route_in("Delegate with `subagent_type: devops`, then run sync-agents."),
        ["subagent_type", "sync-agents"]
    );
    assert!(old_route_in("Dispatch to devops.").is_empty());
}
