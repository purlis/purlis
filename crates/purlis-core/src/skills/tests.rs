// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::updates::Channel;

#[test]
fn the_skill_studio_flag_is_off_by_default_and_on_for_the_dev_channel() {
    let unset = |_: &str| None;
    assert!(!studio_shown(Channel::Stable, &unset));
    assert!(studio_shown(Channel::Dev, &unset));
    assert!(
        !studio_shown(Channel::default(), &unset),
        "stable is the default"
    );
}

#[test]
fn the_variable_turns_the_skill_studio_flag_on_or_off_on_either_channel() {
    let set =
        |value: &'static str| move |name: &str| (name == STUDIO_ENV).then(|| value.to_owned());
    for channel in Channel::ALL {
        assert!(studio_shown(channel, &set("1")), "{channel:?}");
        assert!(!studio_shown(channel, &set("0")), "{channel:?}");
        // A value it does not know leaves the channel to decide.
        assert_eq!(
            studio_shown(channel, &set("yes")),
            channel == Channel::Dev,
            "{channel:?}"
        );
    }
    // Under its old name too, as every product variable is read.
    let old = |name: &str| (name == "CHARTER_SKILL_STUDIO").then(|| "1".to_owned());
    assert!(studio_shown(Channel::Stable, &old));
}

#[test]
fn a_skills_frontmatter_is_read_as_a_personas_is() {
    // purlis-skills keeps its own copy of the frontmatter rule; this holds it to the persona's.
    for text in [
        "---\nname: a\ndescription: b\n---\nbody",
        "---\r\nname:  spaced \r\ndescription:\tTabbed: with a colon\r\n---\r\n",
        "---\nname: a\u{1f}\n\u{2028}description: b\u{85}key-only\n: no key\n---",
        "no frontmatter\n---\nname: a\n---\n",
        "---\nname: unterminated\n",
        "---\n\u{1c}name\u{1d}:\u{1e}x\u{1f}\n---",
    ] {
        assert_eq!(
            purlis_skills::frontmatter(text),
            crate::personas::frontmatter(text),
            "{text:?}"
        );
    }
}

#[test]
fn a_chat_is_handed_every_built_in_skill_by_its_harness_route() {
    let plugin = crate::harness::testing::bundled_plugin();
    let kit = Kit {
        binary: std::path::Path::new("/bin/purlis"),
        plugin: Some(&plugin),
        persona: None,
    };
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        let set = for_chat(kit, harness);

        assert!(
            set.effective.iter().any(|e| e.skill.name == "secrets"),
            "{harness:?}"
        );
        assert!(
            set.effective
                .iter()
                .all(|e| e.route == harness.skills() && e.origin == Origin::BuiltIn),
            "{harness:?}"
        );
        assert!(set.left_out.is_empty(), "{:?}", set.left_out);
        assert_eq!(set.dirs(), [plugin.join("skills").as_path()]);
    }
}
