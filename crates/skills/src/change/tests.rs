// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill's own text, made and changed (SK-7, #2770): names, descriptions, templates, the live
//! lint and the text of a rename.

use super::*;

#[test]
fn a_name_is_lowercase_words_joined_by_single_hyphens() {
    for good in ["deploy", "pr-review", "a", "v2-notes"] {
        assert_eq!(name_problem(good), None, "{good}");
    }
    for (bad, said) in [
        ("", "empty"),
        ("Deploy", "'D'"),
        ("de ploy", "' '"),
        ("-deploy", "hyphen"),
        ("deploy-", "hyphen"),
        ("de--ploy", "two hyphens"),
        ("../x", "'.'"),
    ] {
        let why = name_problem(bad).unwrap_or_else(|| panic!("{bad:?} passed"));
        assert!(why.contains(said), "{bad:?}: {why}");
    }
    assert!(name_problem(&"a".repeat(MOST_NAME)).is_none());
    assert!(name_problem(&"a".repeat(MOST_NAME + 1)).is_some());
}

#[test]
fn a_description_is_one_line_a_harness_reads_as_words() {
    assert_eq!(
        description_problem("Deploys the app, then checks it."),
        None
    );
    for bad in [
        "",
        "  ",
        "two\nlines",
        "Use when: deploying",
        "Deploys # fast",
        "\"quoted\"",
        "- a list",
    ] {
        assert!(description_problem(bad).is_some(), "{bad:?} passed");
    }
    assert!(description_problem(&"a".repeat(MOST_DESCRIPTION + 1)).is_some());
}

#[test]
fn every_template_makes_a_whole_skill_that_keeps_its_name() {
    for template in Template::ALL {
        let text = template.text("deploy", "Deploys the app.");
        assert_eq!(check(&text, "deploy"), Vec::new(), "{template:?}:\n{text}");
        let pairs = crate::frontmatter(&text);
        assert!(pairs.contains(&("name".to_owned(), "deploy".to_owned())));
        assert!(pairs.contains(&("description".to_owned(), "Deploys the app.".to_owned())));
    }
}

#[test]
fn the_lint_says_each_thing_a_save_would_be_refused_for() {
    assert_eq!(check("no frontmatter", "x"), vec![Problem::NoFrontmatter]);
    assert_eq!(
        check("---\ndescription: d\n---\n", "x"),
        vec![Problem::NoName]
    );
    assert_eq!(
        check("---\nname: y\ndescription: d\n---\n", "x"),
        vec![Problem::NameChanged {
            was: "x".to_owned(),
            now: "y".to_owned()
        }]
    );
    assert_eq!(
        check("---\nname: x\n---\n", "x"),
        vec![Problem::NoDescription]
    );
    assert!(matches!(
        check("---\nname: x\ndescription: Use when: x\n---\n", "x").as_slice(),
        [Problem::Description(_)]
    ));
    // Every problem says itself in a line the editor shows.
    assert!(
        Problem::NameChanged {
            was: "x".to_owned(),
            now: "y".to_owned()
        }
        .to_string()
        .contains("Rename")
    );
}

#[test]
fn a_rename_changes_the_name_line_and_nothing_else() {
    let text = "---\nname: deploy\ndescription: Deploys.\n---\n\nname: deploy stays in the body\n";
    assert_eq!(
        with_name(text, "ship").as_deref(),
        Some("---\nname: ship\ndescription: Deploys.\n---\n\nname: deploy stays in the body\n")
    );
    let crlf = "---\r\nname: deploy\r\ndescription: d\r\n---\r\n";
    assert_eq!(
        with_name(crlf, "ship").as_deref(),
        Some("---\r\nname: ship\r\ndescription: d\r\n---\r\n")
    );
    // A line is what the reader calls one (#3244): a lone `\r` ends one too.
    let cr = "---\rname: deploy\rdescription: d\r---\r";
    assert_eq!(
        with_name(cr, "ship").as_deref(),
        Some("---\rname: ship\rdescription: d\r---\r")
    );
    let dashes = "---\ndescription: a --- b\nname: deploy\n---\n";
    assert_eq!(
        with_name(dashes, "ship").as_deref(),
        Some("---\ndescription: a --- b\nname: ship\n---\n")
    );
    assert_eq!(with_name("---\ndescription: d\n---\n", "ship"), None);
    assert_eq!(with_name("no frontmatter", "ship"), None);
}

#[test]
fn a_reference_is_renamed_only_where_it_names_that_skill() {
    let text = "Use skills:deploy first. skills:deployer is another, myskills:deploy is not ours, \
                and (skills:deploy) ends here: skills:deploy";
    assert_eq!(
        references_renamed(text, "skills", "deploy", "ship"),
        "Use skills:ship first. skills:deployer is another, myskills:deploy is not ours, \
         and (skills:ship) ends here: skills:ship"
    );
    assert!(references(text, "skills", "deploy"));
    assert!(!references("skills:deployer only", "skills", "deploy"));
    // Back to back, each is its own reference.
    assert_eq!(
        references_renamed("skills:a skills:a", "skills", "a", "b"),
        "skills:b skills:b"
    );
}

#[test]
fn a_personas_list_follows_a_rename_in_either_spelling() {
    let persona = "---\nname: ops\nskills: deploy, purlis:handoff,skills:deploy , deployer\n---\n\nI deploy.\n";
    let renamed = persona_list_renamed(persona, "skills", "deploy", "ship");
    assert_eq!(
        renamed,
        "---\nname: ops\nskills: ship, purlis:handoff,skills:ship , deployer\n---\n\nI deploy.\n"
    );
    assert!(persona_names(persona, "skills", "deploy"));
    assert!(!persona_names(persona, "skills", "ship"));
    assert!(!persona_names(
        "no frontmatter, skills: deploy",
        "skills",
        "deploy"
    ));
}

#[test]
fn a_personas_block_list_follows_a_rename_too() {
    let persona = "---\nname: ops\nskills:\n  - deploy\n  - skills:deploy\n  - deployer\nrole: Ops\n---\n\n- deploy is prose\n";

    assert_eq!(
        persona_list_renamed(persona, "skills", "deploy", "ship"),
        "---\nname: ops\nskills:\n  - ship\n  - skills:ship\n  - deployer\nrole: Ops\n---\n\n- deploy is prose\n"
    );
    assert!(persona_names(persona, "skills", "deployer"));
    assert!(!persona_names(
        "---\nrole: x\n---\n\n- deploy\n",
        "skills",
        "deploy"
    ));
}
