// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam, for grants, workspace skills and local skills (SK-4, #2767; ADR 0099): what a chat's
//! skill set is, and why. Every test builds a plane in a temporary directory and reads only what
//! the set says, or what Share and a grant leave on disk.

use std::path::{Path, PathBuf};

use super::*;

fn skill(dir: &Path, folder: &str, text: &str) {
    std::fs::create_dir_all(dir.join(folder)).unwrap();
    std::fs::write(dir.join(folder).join("SKILL.md"), text).unwrap();
}

fn whole(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n")
}

/// A plane in a temporary directory, with a workspace `smart-ide` and a workspace `ops`.
struct Plane {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    /// What each workspace turned off, as purlis-core reads it from its settings.
    off: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>>,
}

impl Plane {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("plane");
        for ws in ["smart-ide", "ops"] {
            std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
        }
        Self {
            _tmp: tmp,
            root,
            off: Default::default(),
        }
    }

    fn project(&self) -> PathBuf {
        self.root.join(PROJECT_DIR)
    }

    fn project_local(&self) -> PathBuf {
        self.root.join(LOCAL_DIR)
    }

    fn workspace(&self, ws: &str) -> PathBuf {
        self.root.join("workspaces").join(ws).join(PROJECT_DIR)
    }

    fn workspace_local(&self, ws: &str) -> PathBuf {
        self.root.join("workspaces").join(ws).join(LOCAL_DIR)
    }

    fn grant(&self, folder: &str, text: &str) {
        std::fs::create_dir_all(self.project()).unwrap();
        std::fs::write(self.project().join(format!("{folder}.toml")), text).unwrap();
    }

    fn turn_off(&self, ws: &str, names: &[&str]) {
        self.off.borrow_mut().insert(
            ws.to_owned(),
            names.iter().map(|name| (*name).to_owned()).collect(),
        );
    }

    /// The set of a chat in `workspace` (`None`: at the plane root), with git keeping every
    /// local skill on this machine.
    fn set(&self, workspace: Option<&str>) -> SkillSet {
        self.set_with(workspace, &|_| Ok(()))
    }

    fn set_with(&self, workspace: Option<&str>, kept: &KeptLocal<'_>) -> SkillSet {
        let off = workspace
            .and_then(|ws| self.off.borrow().get(ws).cloned())
            .unwrap_or_default();
        skill_set(
            Some(&self.root),
            &Machine {
                built_in: None,
                kept_local: Some(kept),
                harness: None,
                made_since: None,
            },
            &Chat {
                workspace,
                persona: None,
                names: &[],
                turned_off: &off,
                harness: HarnessSkills {
                    route: Route::Plugin,
                },
            },
        )
    }
}

fn effective(set: &SkillSet) -> Vec<(&str, Origin, bool)> {
    set.effective
        .iter()
        .map(|e| (e.skill.name.as_str(), e.origin, e.local))
        .collect()
}

fn left_out(set: &SkillSet) -> Vec<(&str, Origin, bool, String)> {
    set.left_out
        .iter()
        .map(|l| (l.name.as_str(), l.origin, l.local, l.reason.to_string()))
        .collect()
}

const NOT_GRANTED: &str = "not granted to this workspace: a project skill is for chats at the \
                           project's root until the project grants it";

// ---- grants -------------------------------------------------------------------------------

#[test]
fn a_project_skill_granted_to_all_reaches_every_workspace_and_the_plane_root() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");

    for workspace in [None, Some("smart-ide"), Some("ops")] {
        assert_eq!(
            effective(&plane.set(workspace)),
            [("deploy", Origin::Project, false)],
            "{workspace:?}"
        );
    }
}

#[test]
fn a_project_skill_granted_to_named_workspaces_reaches_only_those() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = [\"smart-ide\"]\n");

    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("deploy", Origin::Project, false)]
    );
    assert_eq!(
        effective(&plane.set(None)),
        [("deploy", Origin::Project, false)],
        "the plane root always has its project's skills"
    );
    let ops = plane.set(Some("ops"));
    assert!(ops.effective.is_empty());
    assert_eq!(
        left_out(&ops),
        [("deploy", Origin::Project, false, NOT_GRANTED.to_owned())]
    );
}

#[test]
fn a_grant_of_root_or_no_grant_file_keeps_a_project_skill_at_the_plane_root() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project(), "lint", &whole("lint"));
    plane.grant("lint", "grant = \"root\"\n");

    let set = plane.set(Some("smart-ide"));

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [
            ("deploy", Origin::Project, false, NOT_GRANTED.to_owned()),
            ("lint", Origin::Project, false, NOT_GRANTED.to_owned()),
        ]
    );
}

#[test]
fn a_grant_that_cannot_be_read_is_read_as_root_and_says_why() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"everyone\"\n");
    skill(&plane.project(), "lint", &whole("lint"));
    plane.grant("lint", "grant = [\n");

    assert_eq!(
        effective(&plane.set(None)),
        [
            ("deploy", Origin::Project, false),
            ("lint", Origin::Project, false)
        ],
        "a grant only ever reaches further than the plane root"
    );
    let reasons: Vec<String> = plane
        .set(Some("smart-ide"))
        .left_out
        .iter()
        .map(|l| l.reason.to_string())
        .collect();
    assert_eq!(
        reasons[0],
        "not granted to this workspace: its grant cannot be read (grant is \"everyone\", which \
         is none of \"root\", \"all\" or a list of workspace names), so it is for chats at the \
         project's root only"
    );
    assert!(
        reasons[1]
            .starts_with("not granted to this workspace: its grant cannot be read (it is not TOML"),
        "{}",
        reasons[1]
    );
}

#[test]
fn a_workspace_turns_a_granted_skill_off_and_its_chats_are_told_so() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project(), "lint", &whole("lint"));
    plane.grant("deploy", "grant = \"all\"\n");
    plane.grant("lint", "grant = \"all\"\n");
    plane.turn_off("smart-ide", &["deploy"]);

    let set = plane.set(Some("smart-ide"));

    assert_eq!(effective(&set), [("lint", Origin::Project, false)]);
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            false,
            "turned off in this workspace".to_owned()
        )]
    );
    assert_eq!(
        effective(&plane.set(Some("ops"))),
        [
            ("deploy", Origin::Project, false),
            ("lint", Origin::Project, false)
        ],
        "one workspace's switch is its own"
    );
    assert_eq!(effective(&plane.set(None)).len(), 2);
}

#[test]
fn a_workspace_cannot_widen_a_grant_by_naming_a_skill_it_was_not_granted() {
    // The workspace's list only turns off: a skill it names that its grant does not reach stays
    // left out, as not granted, and there is no list that turns one on.
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.turn_off("smart-ide", &["deploy"]);

    let set = plane.set(Some("smart-ide"));

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [("deploy", Origin::Project, false, NOT_GRANTED.to_owned())]
    );
}

#[test]
fn grant_of_reads_each_shape_and_reaches_says_where_it_reaches() {
    let plane = Plane::new();
    skill(&plane.project(), "a", &whole("a"));
    let folder = plane.project().join("a");

    assert_eq!(grant_of(&folder), Ok(Grant::Root), "no file");
    plane.grant("a", "grant = \"all\"\n");
    assert_eq!(grant_of(&folder), Ok(Grant::All));
    plane.grant("a", "grant = [\"ops\", \"smart-ide\"]\n");
    let only = grant_of(&folder).unwrap();
    assert_eq!(only, Grant::Only(vec!["ops".into(), "smart-ide".into()]));
    assert!(only.reaches("ops") && only.reaches("smart-ide") && !only.reaches("web"));
    assert_eq!(only.to_string(), "ops, smart-ide");
    assert!(Grant::All.reaches("anything"));
    assert!(!Grant::Root.reaches("smart-ide"));
}

#[test]
fn set_grant_writes_the_grant_beside_the_folder_and_keeps_every_other_key() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    let folder = plane.project().join("deploy");
    let file = plane.project().join("deploy.toml");
    plane.grant("deploy", "# where it came from\nsource = \"made here\"\n");

    set_grant(&folder, &Grant::Only(vec!["smart-ide".into()])).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "# where it came from\nsource = \"made here\"\ngrant = [\"smart-ide\"]\n"
    );
    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("deploy", Origin::Project, false)]
    );

    set_grant(&folder, &Grant::All).unwrap();
    assert_eq!(grant_of(&folder), Ok(Grant::All));

    set_grant(&folder, &Grant::Root).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "# where it came from\nsource = \"made here\"\n",
        "root is the default: the key goes"
    );
}

#[test]
fn set_grant_of_root_on_a_file_holding_only_the_grant_removes_the_file() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    let folder = plane.project().join("deploy");
    set_grant(&folder, &Grant::All).unwrap();
    assert!(plane.project().join("deploy.toml").is_file());

    set_grant(&folder, &Grant::Root).unwrap();

    assert!(!plane.project().join("deploy.toml").exists());
    assert_eq!(grant_of(&folder), Ok(Grant::Root));
}

#[test]
fn set_grant_refuses_a_file_that_is_not_toml_and_leaves_it_as_it_was() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = [\n");

    let refused = set_grant(&plane.project().join("deploy"), &Grant::All).unwrap_err();

    assert!(refused.contains("deploy.toml is not TOML"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(plane.project().join("deploy.toml")).unwrap(),
        "grant = [\n"
    );
}

// ---- workspace skills ---------------------------------------------------------------------

#[test]
fn a_workspace_skill_reaches_only_that_workspaces_chats() {
    let plane = Plane::new();
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));

    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("review", Origin::Workspace, false)]
    );
    assert_eq!(
        plane.set(Some("smart-ide")).effective[0].skill.path,
        plane.workspace("smart-ide").join("review/SKILL.md")
    );
    assert_eq!(
        plane.set(Some("ops")),
        SkillSet::default(),
        "never another's"
    );
    assert_eq!(plane.set(None), SkillSet::default(), "nor the plane root's");
}

#[test]
fn a_workspaces_turn_off_list_does_not_reach_its_own_skills() {
    let plane = Plane::new();
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));
    plane.turn_off("smart-ide", &["review"]);

    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("review", Origin::Workspace, false)]
    );
}

#[test]
fn a_workspace_name_that_is_not_one_plain_name_reads_no_workspace_level() {
    let plane = Plane::new();
    skill(&plane.root.join("skills"), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));

    let set = plane.set(Some("../plane/workspaces/smart-ide"));

    assert!(set.effective.is_empty(), "{:?}", set.effective);
    assert_eq!(
        left_out(&set),
        [("deploy", Origin::Project, false, NOT_GRANTED.to_owned())]
    );
}

// ---- local skills -------------------------------------------------------------------------

#[test]
fn a_local_skill_reaches_chats_at_its_level_while_git_keeps_it_on_this_machine() {
    let plane = Plane::new();
    skill(&plane.project_local(), "scratch", &whole("scratch"));
    skill(
        &plane.workspace_local("smart-ide"),
        "notes",
        &whole("notes"),
    );

    assert_eq!(
        effective(&plane.set(None)),
        [("scratch", Origin::Project, true)]
    );
    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("notes", Origin::Workspace, true)],
        "a local project skill reaches a workspace by its grant, as a shared one does"
    );
    assert_eq!(
        plane.set(Some("smart-ide")).effective[0].skill.path,
        plane.workspace_local("smart-ide").join("notes/SKILL.md")
    );
}

#[test]
fn a_local_project_skill_reaches_a_workspace_by_its_own_grant() {
    let plane = Plane::new();
    skill(&plane.project_local(), "scratch", &whole("scratch"));
    std::fs::write(
        plane.project_local().join("scratch.toml"),
        "grant = \"all\"\n",
    )
    .unwrap();

    assert_eq!(
        effective(&plane.set(Some("ops"))),
        [("scratch", Origin::Project, true)]
    );
}

#[test]
fn a_local_skill_git_would_carry_is_left_out_with_gits_answer() {
    let plane = Plane::new();
    skill(&plane.project_local(), "scratch", &whole("scratch"));
    let asked = std::sync::Mutex::new(Vec::new());
    let kept = |folder: &Path| -> Result<(), String> {
        asked.lock().unwrap().push(folder.to_path_buf());
        Err("git would commit it".to_owned())
    };

    let set = plane.set_with(None, &kept);

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [(
            "scratch",
            Origin::Project,
            true,
            "a local skill is used only while it stays on this machine, and git would commit it: \
             purlis reinit keeps it here"
                .to_owned()
        )]
    );
    assert_eq!(
        *asked.lock().unwrap(),
        [plane.project_local().join("scratch")]
    );
}

#[test]
fn without_a_way_to_ask_git_no_local_skill_is_read() {
    let plane = Plane::new();
    skill(&plane.project_local(), "scratch", &whole("scratch"));

    let set = skill_set(
        Some(&plane.root),
        &Machine::default(),
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Config,
            },
        },
    );

    assert!(set.effective.is_empty());
    assert_eq!(set.left_out.len(), 1);
    assert!(
        set.left_out[0]
            .reason
            .to_string()
            .contains("purlis could not ask git whether it does"),
        "{}",
        set.left_out[0].reason
    );
}

// ---- narrowest wins -----------------------------------------------------------------------

#[test]
fn the_narrowest_copy_of_a_name_wins_and_every_hidden_copy_says_which_hid_it() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    skill(&plane.project_local(), "deploy", &whole("deploy"));
    std::fs::write(
        plane.project_local().join("deploy.toml"),
        "grant = \"all\"\n",
    )
    .unwrap();
    skill(&plane.workspace("smart-ide"), "deploy", &whole("deploy"));
    skill(
        &plane.workspace_local("smart-ide"),
        "deploy",
        &whole("deploy"),
    );

    let set = plane.set(Some("smart-ide"));

    assert_eq!(effective(&set), [("deploy", Origin::Workspace, true)]);
    assert_eq!(
        set.effective[0].skill.path,
        plane.workspace_local("smart-ide").join("deploy/SKILL.md")
    );
    let hidden: Vec<(Origin, bool, String, PathBuf)> = set
        .left_out
        .iter()
        .map(|l| (l.origin, l.local, l.reason.to_string(), l.folder.clone()))
        .collect();
    let by = "hidden by the local workspace skill of this name".to_owned();
    assert_eq!(
        hidden,
        [
            (
                Origin::Project,
                false,
                by.clone(),
                plane.project().join("deploy")
            ),
            (
                Origin::Project,
                true,
                by.clone(),
                plane.project_local().join("deploy")
            ),
            (
                Origin::Workspace,
                false,
                by,
                plane.workspace("smart-ide").join("deploy")
            ),
        ],
        "every hidden copy is listed, widest first"
    );
}

#[test]
fn at_one_level_a_local_copy_hides_the_shared_one() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project_local(), "deploy", &whole("deploy"));

    let set = plane.set(None);

    assert_eq!(effective(&set), [("deploy", Origin::Project, true)]);
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            false,
            "hidden by the local project skill of this name".to_owned()
        )]
    );
}

#[test]
fn a_workspace_skill_hides_a_project_skill_of_its_name_whether_granted_or_not() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.workspace("smart-ide"), "deploy", &whole("deploy"));

    let set = plane.set(Some("smart-ide"));

    assert_eq!(effective(&set), [("deploy", Origin::Workspace, false)]);
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            false,
            "hidden by the workspace skill of this name".to_owned()
        )]
    );
}

#[test]
fn a_narrower_copy_that_is_left_out_hides_nothing() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project_local(), "deploy", &whole("deploy"));

    let set = plane.set_with(None, &|_| Err("git would commit it".to_owned()));

    assert_eq!(effective(&set), [("deploy", Origin::Project, false)]);
    assert_eq!(set.left_out.len(), 1);
    assert!(set.left_out[0].local);
}

#[test]
fn a_workspace_or_local_skill_never_takes_a_built_in_skills_name() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "handoff", &whole("handoff"));
    skill(
        &plane.workspace_local("smart-ide"),
        "handoff",
        &whole("handoff"),
    );

    let set = skill_set(
        Some(&plane.root),
        &Machine {
            built_in: Some(&built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
            made_since: None,
        },
        &Chat {
            workspace: Some("smart-ide"),
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Plugin,
            },
        },
    );

    assert_eq!(effective(&set), [("handoff", Origin::BuiltIn, false)]);
    assert_eq!(
        left_out(&set)[0].3,
        "a built-in skill has this name, and a built-in skill is never replaced"
    );
}

/// The toml a link, so the folder is put back, and its way back taken first: share says the
/// folder moved and the file beside it did not, never "nothing was moved" (SK-45).
#[cfg(unix)]
#[test]
fn share_whose_folder_cannot_be_put_back_says_what_moved() {
    let plane = Plane::new();
    let outside = plane._tmp.path().join("outside.toml");
    std::fs::write(&outside, "grant = \"all\"\n").unwrap();
    skill(&plane.project_local(), "notes", &whole("notes"));
    std::os::unix::fs::symlink(&outside, plane.project_local().join("notes.toml")).unwrap();
    let back_to = plane.project_local().join("notes");
    let _hook = crate::moving::hook::set(move |moment, _, to| {
        if moment == crate::moving::hook::Moment::BeforeRename && to == back_to {
            std::fs::create_dir(to).unwrap();
        }
    });

    let refused = share(&plane.root, &Whose::Project, "notes").unwrap_err();

    assert_eq!(
        refused,
        "notes could not be moved to skills/notes: its folder was moved to skills/notes and \
         could not be put back (something of its name is already where it goes), and \
         skills.local/notes.toml was not moved (it is not a plain folder or file, so purlis \
         does not move it)"
    );
    assert!(plane.project().join("notes/SKILL.md").is_file());
}

// ---- share --------------------------------------------------------------------------------

#[test]
fn share_moves_a_local_project_skill_and_its_grant_into_the_shared_folder() {
    let plane = Plane::new();
    skill(&plane.project_local(), "scratch-folder", &whole("scratch"));
    std::fs::write(
        plane.project_local().join("scratch-folder.toml"),
        "grant = \"all\"\n",
    )
    .unwrap();

    let shared = share(&plane.root, &Whose::Project, "scratch").unwrap();
    let moved = shared.to;

    assert_eq!(shared.from, plane.project_local().join("scratch-folder"));
    assert_eq!(moved, plane.project().join("scratch-folder"));
    assert!(moved.join("SKILL.md").is_file());
    assert!(!plane.project_local().join("scratch-folder").exists());
    assert_eq!(
        std::fs::read_to_string(plane.project().join("scratch-folder.toml")).unwrap(),
        "grant = \"all\"\n"
    );
    assert_eq!(
        effective(&plane.set(Some("ops"))),
        [("scratch", Origin::Project, false)],
        "shared, it is an ordinary project skill with the grant it had"
    );
}

#[test]
fn share_moves_a_local_workspace_skill_into_that_workspaces_skills() {
    let plane = Plane::new();
    skill(
        &plane.workspace_local("smart-ide"),
        "notes",
        &whole("notes"),
    );

    let moved = share(&plane.root, &Whose::Workspace("smart-ide".into()), "notes")
        .unwrap()
        .to;

    assert_eq!(moved, plane.workspace("smart-ide").join("notes"));
    assert_eq!(
        effective(&plane.set(Some("smart-ide"))),
        [("notes", Origin::Workspace, false)]
    );
}

#[test]
fn share_refuses_when_the_shared_folder_is_taken_and_moves_nothing() {
    let plane = Plane::new();
    skill(&plane.project_local(), "deploy", &whole("deploy"));
    skill(&plane.project(), "deploy", &whole("deploy"));

    let refused = share(&plane.root, &Whose::Project, "deploy").unwrap_err();

    assert_eq!(
        refused,
        "skills/deploy is already there, so nothing was moved: rename one of the two folders \
         first"
    );
    assert!(plane.project_local().join("deploy/SKILL.md").is_file());
}

#[test]
fn share_of_a_name_no_local_skill_has_says_so() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));

    assert_eq!(
        share(&plane.root, &Whose::Project, "deploy").unwrap_err(),
        "no local skill is named \"deploy\" at the project's root"
    );
    assert_eq!(
        share(&plane.root, &Whose::Workspace("smart-ide".into()), "deploy").unwrap_err(),
        "no local skill is named \"deploy\" in workspace smart-ide"
    );
}

#[test]
fn named_finds_a_skills_folder_by_its_name_not_its_folders() {
    let plane = Plane::new();
    skill(&plane.project(), "the-deploy-folder", &whole("deploy"));

    assert_eq!(
        named(&plane.project(), "deploy"),
        Some(plane.project().join("the-deploy-folder"))
    );
    assert_eq!(named(&plane.project(), "the-deploy-folder"), None);
}

#[test]
fn the_words_of_a_grant_are_read_by_the_grant_files_rule() {
    let words = |list: &[&str]| -> Vec<String> { list.iter().map(|w| (*w).to_owned()).collect() };
    assert_eq!(Grant::from_words(&words(&["root"])), Ok(Grant::Root));
    assert_eq!(Grant::from_words(&words(&["all"])), Ok(Grant::All));
    assert_eq!(
        Grant::from_words(&words(&["ops", "web"])),
        Ok(Grant::Only(words(&["ops", "web"])))
    );
    assert!(Grant::from_words(&[]).is_err());
    assert!(Grant::from_words(&words(&["all", "ops"])).is_err());
    assert!(Grant::from_words(&words(&["../x"])).is_err());
}

// ---- persona skills (SK-5, #2768) ----------------------------------------------------------

impl Plane {
    fn persona(&self, persona: &str) -> PathBuf {
        self.root.join("personas").join(persona).join(PROJECT_DIR)
    }

    fn persona_local(&self, persona: &str) -> PathBuf {
        self.root.join("personas").join(persona).join(LOCAL_DIR)
    }

    /// The set of a chat as `persona`, whose `skills:` list says `names`, in `workspace`, with
    /// the built-in skills in `built_in` and the harness skills `harness` on this machine.
    fn as_persona(
        &self,
        workspace: Option<&str>,
        persona: &str,
        names: &[&str],
        built_in: Option<&Path>,
        harness: Option<&[OnMachine]>,
    ) -> SkillSet {
        let names: Vec<Reference> = names.iter().filter_map(|n| Reference::parse(n)).collect();
        let off = workspace
            .and_then(|ws| self.off.borrow().get(ws).cloned())
            .unwrap_or_default();
        skill_set(
            Some(&self.root),
            &Machine {
                built_in,
                kept_local: Some(&|_| Ok(())),
                harness,
                made_since: None,
            },
            &Chat {
                workspace,
                persona: Some(persona),
                names: &names,
                turned_off: &off,
                harness: HarnessSkills {
                    route: Route::Plugin,
                },
            },
        )
    }
}

#[test]
fn a_personas_own_skills_reach_its_chats_in_every_workspace_and_at_the_root() {
    let plane = Plane::new();
    skill(&plane.persona("steward"), "triage", &whole("triage"));
    skill(&plane.persona_local("steward"), "notes", &whole("notes"));

    for workspace in [None, Some("smart-ide"), Some("ops")] {
        let set = plane.as_persona(workspace, "steward", &[], None, None);
        assert_eq!(
            effective(&set),
            [
                ("notes", Origin::Persona, true),
                ("triage", Origin::Persona, false)
            ],
            "{workspace:?}"
        );
    }
}

#[test]
fn another_personas_skills_and_a_chat_on_no_persona_get_none_of_them() {
    let plane = Plane::new();
    skill(&plane.persona("steward"), "triage", &whole("triage"));

    assert!(
        plane
            .as_persona(None, "ops", &[], None, None)
            .effective
            .is_empty()
    );
    assert!(plane.set(Some("smart-ide")).effective.is_empty());
}

#[test]
fn a_project_skill_the_persona_names_reaches_a_workspace_it_was_not_granted_to() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project(), "review", &whole("review"));

    let set = plane.as_persona(Some("smart-ide"), "steward", &["deploy"], None, None);

    assert_eq!(effective(&set), [("deploy", Origin::Project, false)]);
    assert_eq!(
        left_out(&set),
        [("review", Origin::Project, false, NOT_GRANTED.to_owned())],
        "a skill it does not name is still held to its grant"
    );
}

#[test]
fn a_skill_the_persona_names_reaches_a_workspace_that_turned_it_off() {
    // Dispatching a persona never strips its skills (spec story 17; ADR 0099 as amended
    // 2026-10-21): the turn-off still holds for every other chat there.
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    plane.turn_off("ops", &["deploy"]);

    let named = plane.as_persona(Some("ops"), "steward", &["deploy"], None, None);
    let other = plane.as_persona(Some("ops"), "steward", &[], None, None);

    assert_eq!(effective(&named), [("deploy", Origin::Project, false)]);
    assert_eq!(left_out(&other)[0].3, "turned off in this workspace");
}

#[test]
fn the_persona_copy_beats_the_workspace_copy_which_beats_the_project_copy() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    skill(&plane.workspace("smart-ide"), "deploy", &whole("deploy"));
    skill(&plane.persona("steward"), "deploy", &whole("deploy"));

    let set = plane.as_persona(Some("smart-ide"), "steward", &["deploy"], None, None);

    assert_eq!(effective(&set), [("deploy", Origin::Persona, false)]);
    assert_eq!(
        left_out(&set),
        [
            (
                "deploy",
                Origin::Project,
                false,
                "hidden by the persona skill of this name".to_owned()
            ),
            (
                "deploy",
                Origin::Workspace,
                false,
                "hidden by the persona skill of this name".to_owned()
            ),
        ]
    );
}

#[test]
fn a_persona_skill_never_takes_a_built_in_skills_name() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "handoff", &whole("handoff"));
    skill(&plane.persona("steward"), "handoff", &whole("handoff"));

    let set = plane.as_persona(None, "steward", &[], Some(&built_in), None);

    assert_eq!(effective(&set), [("handoff", Origin::BuiltIn, false)]);
    assert_eq!(left_out(&set)[0].1, Origin::Persona);
}

#[test]
fn a_built_in_skill_named_with_either_prefix_is_the_built_in_one_and_a_missing_one_says_so() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(
        &built_in,
        "working-in-a-clone",
        &whole("working-in-a-clone"),
    );

    let set = plane.as_persona(
        None,
        "steward",
        &[
            "charter:working-in-a-clone",
            "purlis:working-in-a-clone",
            "purlis:gone",
        ],
        Some(&built_in),
        Some(&[]),
    );

    assert_eq!(
        effective(&set),
        [("working-in-a-clone", Origin::BuiltIn, false)]
    );
    assert_eq!(
        left_out(&set),
        [(
            "purlis:gone",
            Origin::BuiltIn,
            false,
            "its persona names it, and purlis has no built-in skill of this name".to_owned()
        )]
    );
}

#[test]
fn a_harness_skill_the_persona_names_is_left_out_only_where_this_machine_lacks_it() {
    let plane = Plane::new();
    let here = [
        OnMachine::Skill {
            plugin: Some("mattpocock-skills".to_owned()),
            name: "grilling".to_owned(),
        },
        OnMachine::Skill {
            plugin: None,
            name: "langfuse".to_owned(),
        },
        OnMachine::Unlisted("superpowers".to_owned()),
    ];
    let names = [
        "mattpocock-skills:grilling",
        "mattpocock-skills:tdd",
        "superpowers:systematic-debugging",
        "langfuse",
        "nowhere",
    ];

    let set = plane.as_persona(None, "steward", &names, None, Some(&here));

    assert!(set.effective.is_empty(), "the harness loads its own");
    let harness: Vec<(String, bool)> = set
        .from_the_harness
        .iter()
        .map(|h| (h.reference.to_string(), h.checked))
        .collect();
    assert_eq!(
        harness,
        [
            ("mattpocock-skills:grilling".to_owned(), true),
            ("superpowers:systematic-debugging".to_owned(), false),
            ("langfuse".to_owned(), true),
        ],
        "a skill of a plugin it cannot list is never said to be there"
    );
    let missing = "its persona names it, and it is not installed on this machine".to_owned();
    assert_eq!(
        left_out(&set),
        [
            (
                "mattpocock-skills:tdd",
                Origin::Harness,
                false,
                missing.clone()
            ),
            ("nowhere", Origin::Harness, false, missing),
        ]
    );
}

#[test]
fn where_nobody_looked_at_the_harness_no_named_skill_is_missing_or_checked() {
    let plane = Plane::new();

    let set = plane.as_persona(None, "steward", &["superpowers:tdd", "nowhere"], None, None);

    assert!(set.left_out.is_empty());
    assert!(set.from_the_harness.iter().all(|h| !h.checked));
    assert_eq!(set.from_the_harness.len(), 2);
}

#[test]
fn a_plane_skill_the_persona_names_is_never_also_looked_for_on_the_machine() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project(), "broken", "no frontmatter\n");

    let set = plane.as_persona(
        Some("ops"),
        "steward",
        &["deploy", "broken"],
        None,
        Some(&[]),
    );

    assert_eq!(effective(&set), [("deploy", Origin::Project, false)]);
    assert_eq!(left_out(&set).len(), 1, "{:?}", left_out(&set));
    assert_eq!(left_out(&set)[0].0, "broken");
    assert_eq!(left_out(&set)[0].1, Origin::Project);
}

#[test]
fn a_reference_is_read_by_its_prefix() {
    assert_eq!(
        Reference::parse(" charter:handoff "),
        Some(Reference::BuiltIn("handoff".to_owned()))
    );
    assert_eq!(
        Reference::parse("purlis:handoff").map(|r| r.to_string()),
        Some("purlis:handoff".to_owned())
    );
    assert_eq!(
        Reference::parse("charter:handoff").map(|r| r.to_string()),
        Some("purlis:handoff".to_owned()),
        "the old prefix is said as the new one"
    );
    assert_eq!(
        Reference::parse("superpowers:tdd"),
        Some(Reference::Harness {
            plugin: "superpowers".to_owned(),
            name: "tdd".to_owned()
        })
    );
    assert_eq!(
        Reference::parse("deploy"),
        Some(Reference::Named("deploy".to_owned()))
    );
    for nothing in ["", "  ", ":tdd", "superpowers:", ":"] {
        assert_eq!(Reference::parse(nothing), None, "{nothing:?}");
    }
}

#[test]
fn a_persona_name_that_is_not_one_plain_name_has_no_folder() {
    let plane = Plane::new();
    assert_eq!(persona_dir(&plane.root, "../x", false), None);
    assert_eq!(
        persona_dir(&plane.root, "steward", true),
        Some(plane.root.join("personas/steward/skills.local"))
    );
}

// ---- harness and repo skills (SK-6, #2769) ------------------------------------------------

fn outside(id: &str, origin: Origin, owner: Owner, off: bool) -> OutsideSkill {
    OutsideSkill {
        skill: Skill {
            name: id.rsplit(':').next().unwrap_or(id).to_owned(),
            description: format!("Does {id}."),
            path: PathBuf::from(format!("/elsewhere/{id}/SKILL.md")),
        },
        root: PathBuf::from(format!("/elsewhere/{id}")),
        origin,
        id: id.to_owned(),
        owner,
        switch: Switch::Alone,
        off,
        unread: None,
    }
}

fn repo(name: &str) -> Owner {
    Owner::Repo(name.to_owned())
}

#[test]
fn a_skill_from_outside_the_plane_is_listed_by_the_name_its_harness_knows_and_handed_by_none_of_purliss_routes()
 {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));

    let set = plane.set(None).with_outside(
        Route::Plugin,
        [
            outside(
                "figma:frame",
                Origin::Harness,
                Owner::Plugin("figma".to_owned()),
                false,
            ),
            outside("review", Origin::Repo, repo("api"), false),
        ],
    );

    assert_eq!(
        effective(&set),
        [
            ("deploy", Origin::Project, false),
            ("figma:frame", Origin::Harness, false),
            ("review", Origin::Repo, false),
        ]
    );
    let routes: Vec<Route> = set.effective.iter().map(|e| e.route).collect();
    assert_eq!(routes, [Route::Plugin, Route::Own, Route::Own]);
    let handed: Vec<&str> = set
        .from_the_plane()
        .map(|e| e.skill.name.as_str())
        .collect();
    assert_eq!(handed, ["deploy"], "purlis hands only the plane's own");
    assert!(set.clashes.is_empty());
}

#[test]
fn a_harness_skill_the_project_turned_off_is_left_out_saying_so() {
    let plane = Plane::new();

    let set = plane.set(None).with_outside(
        Route::Plugin,
        [outside("langfuse", Origin::Harness, Owner::You, true)],
    );

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [(
            "langfuse",
            Origin::Harness,
            false,
            "turned off for this project's chats".to_owned()
        )]
    );
}

#[test]
fn on_a_plugin_route_a_repo_skill_of_a_plane_skills_name_loads_beside_it_and_is_marked() {
    let plane = Plane::new();
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));

    let set = plane.set(Some("smart-ide")).with_outside(
        Route::Plugin,
        [outside("review", Origin::Repo, repo("api"), false)],
    );

    assert_eq!(
        effective(&set),
        [
            ("review", Origin::Workspace, false),
            ("review", Origin::Repo, false)
        ],
        "the plane's copy is its own namespaced plugin, so both load"
    );
    assert_eq!(
        set.clashes,
        [Clash {
            name: "review".to_owned(),
            plane: At {
                origin: Origin::Workspace,
                local: false
            },
            outside: Origin::Repo,
            owner: repo("api"),
            outcome: Outcome::BothLoad,
        }]
    );
}

#[test]
fn where_purlis_builds_what_the_harness_is_handed_the_narrower_repo_copy_wins() {
    let plane = Plane::new();
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));

    for route in [Route::Config, Route::Briefing] {
        let set = plane
            .set(Some("smart-ide"))
            .with_outside(route, [outside("review", Origin::Repo, repo("api"), false)]);

        assert_eq!(
            effective(&set),
            [("review", Origin::Repo, false)],
            "{route:?}"
        );
        assert_eq!(
            left_out(&set),
            [(
                "review",
                Origin::Workspace,
                false,
                "hidden by the repo skill of this name".to_owned()
            )],
            "{route:?}"
        );
        assert_eq!(
            set.from_the_plane().count(),
            0,
            "{route:?}: one copy reaches the chat"
        );
        assert_eq!(set.clashes[0].outcome, Outcome::RepoWins, "{route:?}");
    }
}

#[test]
fn a_harness_skill_is_wider_than_the_planes_so_both_load_on_every_route() {
    let plane = Plane::new();
    skill(&plane.project(), "review", &whole("review"));

    let set = plane.set(None).with_outside(
        Route::Config,
        [outside("review", Origin::Harness, Owner::You, false)],
    );

    assert_eq!(set.from_the_plane().count(), 1);
    assert_eq!(set.clashes[0].outcome, Outcome::BothLoad);
}

#[test]
fn a_skill_turned_off_clashes_with_nothing_and_a_plugins_namespaced_skill_never_clashes() {
    let plane = Plane::new();
    skill(&plane.project(), "frame", &whole("frame"));

    let set = plane.set(None).with_outside(
        Route::Config,
        [
            outside("frame", Origin::Repo, repo("api"), true),
            outside(
                "figma:frame",
                Origin::Harness,
                Owner::Plugin("figma".to_owned()),
                false,
            ),
        ],
    );

    assert!(set.clashes.is_empty(), "{:?}", set.clashes);
    assert_eq!(set.from_the_plane().count(), 1);
}

#[test]
fn a_built_in_skill_never_loses_to_a_repo_or_harness_copy_of_its_name_on_any_route() {
    // SK-6 (#2769): a clone could otherwise replace purlis's own `secrets` on Codex and opencode.
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "secrets", &whole("secrets"));
    // The config route names it `purlis-secrets`, which no copy here has
    // (`a_built_in_skill_on_the_config_route_goes_by_a_name_of_its_own`).
    for route in [Route::Plugin, Route::Briefing] {
        let got = built_in_set(&plane, &built_in, route).with_outside(
            route,
            [
                outside("secrets", Origin::Repo, repo("api"), false),
                outside("secrets", Origin::Harness, Owner::You, false),
            ],
        );
        assert_eq!(
            effective(&got),
            [("secrets", Origin::BuiltIn, false)],
            "{route:?}"
        );
        let hidden: Vec<String> = got.left_out.iter().map(|l| l.reason.to_string()).collect();
        assert_eq!(
            hidden, ["hidden by purlis's built-in skill of this name"; 2],
            "{route:?}"
        );
        assert!(got.clashes.is_empty(), "{route:?}");
    }
}

/// The set of a chat at `plane`'s root on `route`, with the built-in skills in `built_in`.
fn built_in_set(plane: &Plane, built_in: &Path, route: Route) -> SkillSet {
    skill_set(
        Some(&plane.root),
        &Machine {
            built_in: Some(built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
            made_since: None,
        },
        &Chat {
            workspace: None,
            persona: None,
            names: &[],
            turned_off: &[],
            harness: HarnessSkills { route },
        },
    )
}

#[test]
fn a_built_in_skill_on_the_config_route_goes_by_a_name_of_its_own() {
    // opencode's route: the harness reads every skill by its bare name, purlis's and the clone's
    // alike, so purlis's `secrets` is `purlis-secrets` there and nowhere else.
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "secrets", &whole("secrets"));

    let set = built_in_set(&plane, &built_in, Route::Config);
    assert_eq!(
        effective(&set),
        [("purlis-secrets", Origin::BuiltIn, false)]
    );
    assert_eq!(
        set.effective[0].skill.path,
        built_in.join("secrets/SKILL.md"),
        "still the bundle's own skill"
    );
    assert_eq!(
        set.in_the_folder()
            .map(|e| e.skill.name.as_str())
            .collect::<Vec<_>>(),
        ["purlis-secrets"],
        "the set's folder holds it, under that name"
    );

    for route in [Route::Plugin, Route::Briefing, Route::Mcp] {
        let set = built_in_set(&plane, &built_in, route);
        assert_eq!(
            effective(&set),
            [("secrets", Origin::BuiltIn, false)],
            "{route:?} is as it was"
        );
        assert_eq!(set.in_the_folder().count(), 0, "{route:?}");
    }
}

#[test]
fn on_the_config_route_a_repo_or_harness_skill_of_a_built_in_skills_bare_name_is_another_skill() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "secrets", &whole("secrets"));

    let got = built_in_set(&plane, &built_in, Route::Config).with_outside(
        Route::Config,
        [
            outside("secrets", Origin::Repo, repo("api"), false),
            outside("secrets", Origin::Harness, Owner::You, false),
        ],
    );

    assert_eq!(
        effective(&got),
        [
            ("purlis-secrets", Origin::BuiltIn, false),
            ("secrets", Origin::Repo, false),
            ("secrets", Origin::Harness, false),
        ],
        "both are in the set, each by its own name"
    );
    assert!(got.left_out.is_empty(), "{:?}", got.left_out);
    assert!(got.clashes.is_empty(), "{:?}", got.clashes);
}

#[test]
fn on_the_config_route_a_built_in_skills_own_name_is_kept_for_it_from_every_level() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "secrets", &whole("secrets"));
    skill(&plane.project(), "purlis-secrets", &whole("purlis-secrets"));
    skill(&plane.project(), "secrets", &whole("secrets"));
    skill(&plane.project(), "purlis-deploy", &whole("purlis-deploy"));

    let got = built_in_set(&plane, &built_in, Route::Config).with_outside(
        Route::Config,
        [
            outside("purlis-secrets", Origin::Repo, repo("api"), false),
            outside("purlis-secrets", Origin::Harness, Owner::You, false),
        ],
    );

    assert_eq!(
        effective(&got),
        [
            ("purlis-deploy", Origin::Project, false),
            ("purlis-secrets", Origin::BuiltIn, false),
        ],
        "a `purlis-` name no built-in skill takes is the plane's to use"
    );
    let kept = "purlis's built-in skill goes by this name in this program, and the name is kept \
                for it: rename this skill";
    assert_eq!(
        left_out(&got),
        [
            ("purlis-secrets", Origin::Project, false, kept.to_owned()),
            ("purlis-secrets", Origin::Harness, false, kept.to_owned()),
            ("purlis-secrets", Origin::Repo, false, kept.to_owned()),
            (
                "secrets",
                Origin::Project,
                false,
                "a built-in skill has this name, and a built-in skill is never replaced".to_owned()
            ),
        ],
        "one reason for every level; the bare name stays the built-in skill's, as on every route"
    );

    // Elsewhere a plane skill may be named so: Claude Code knows it as `skills:purlis-secrets`.
    let plugin = built_in_set(&plane, &built_in, Route::Plugin);
    assert!(
        effective(&plugin).contains(&("purlis-secrets", Origin::Project, false)),
        "{:?}",
        plugin.left_out
    );
}

#[test]
fn a_persona_names_a_built_in_skill_by_its_bare_name_on_the_config_route_too() {
    let plane = Plane::new();
    let built_in = plane.root.join("../plugin/skills");
    skill(&built_in, "secrets", &whole("secrets"));
    let names = [Reference::BuiltIn("secrets".to_owned())];

    let set = skill_set(
        Some(&plane.root),
        &Machine {
            built_in: Some(&built_in),
            kept_local: Some(&|_| Ok(())),
            harness: None,
            made_since: None,
        },
        &Chat {
            workspace: None,
            persona: Some("reviewer"),
            names: &names,
            turned_off: &[],
            harness: HarnessSkills {
                route: Route::Config,
            },
        },
    );

    assert_eq!(
        effective(&set),
        [("purlis-secrets", Origin::BuiltIn, false)]
    );
    assert!(
        set.left_out.is_empty(),
        "never \"no such built-in\": {:?}",
        set.left_out
    );
}

// ---- lifecycle (SK-19, #2782) -------------------------------------------------------------

#[test]
fn an_archived_skill_never_loads_and_its_left_out_line_says_so() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\nlifecycle = \"archived\"\n");

    for workspace in [None, Some("ops")] {
        let set = plane.set(workspace);
        assert!(effective(&set).is_empty(), "{workspace:?}");
        assert_eq!(
            left_out(&set),
            [(
                "deploy",
                Origin::Project,
                false,
                "archived: it never loads until it is restored from its tab".to_owned()
            )]
        );
    }
}

#[test]
fn an_archived_copy_takes_no_name_from_a_wider_copy() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    skill(&plane.workspace("ops"), "deploy", &whole("deploy"));
    std::fs::write(
        plane.workspace("ops").join("deploy.toml"),
        "lifecycle = \"archived\"\n",
    )
    .unwrap();

    let set = plane.set(Some("ops"));

    assert_eq!(effective(&set), [("deploy", Origin::Project, false)]);
    assert_eq!(set.left_out[0].reason, Reason::Archived);
}

#[test]
fn a_deprecated_skill_still_loads_and_the_set_records_its_replacement() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    skill(&plane.project(), "ship", &whole("ship"));
    plane.grant(
        "deploy",
        "lifecycle = \"deprecated\"\nreplacement = \"ship\"\n",
    );

    let set = plane.set(None);

    assert_eq!(
        effective(&set),
        [
            ("deploy", Origin::Project, false),
            ("ship", Origin::Project, false)
        ]
    );
    assert_eq!(
        set.effective[0].deprecated,
        Some(Deprecated {
            replacement: Some("ship".to_owned())
        })
    );
    assert_eq!(set.effective[1].deprecated, None);
}

#[test]
fn a_lifecycle_word_purlis_does_not_know_leaves_the_skill_working() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "lifecycle = \"retired\"\n");

    let set = plane.set(None);

    assert_eq!(effective(&set), [("deploy", Origin::Project, false)]);
    assert_eq!(set.effective[0].deprecated, None);
    assert!(
        about_of(&plane.project().join("deploy"))
            .unwrap_err()
            .contains("none of \"active\", \"deprecated\" or \"archived\"")
    );
}

#[test]
fn every_levels_local_folder_is_spelled_by_the_one_rule() {
    // SK-32 (#3003, #3124): a persona's level is one more, and every pattern follows it.
    assert_eq!(
        local_globs(),
        [
            "skills.local",
            "workspaces/*/skills.local",
            "personas/*/skills.local"
        ]
    );
    assert_eq!(
        level_of_rel("personas/reviewer/skills.local"),
        Some((Whose::Persona("reviewer".to_owned()), true))
    );
    assert_eq!(level_of_rel("personas/a/b/skills"), None);
    for inside in [
        "skills.local",
        "skills.local/x/SKILL.md",
        "workspaces/ops/skills.local/x/SKILL.md",
        "personas/reviewer/skills.local/x/SKILL.md",
    ] {
        assert!(in_local_level(inside), "{inside}");
    }
    for outside in [
        "skills/x/SKILL.md",
        "personas/reviewer/skills/x/SKILL.md",
        "personas/reviewer/persona.md",
        "notes/skills.local/x",
    ] {
        assert!(!in_local_level(outside), "{outside}");
    }
}

#[test]
fn an_owner_reads_as_you_its_plugin_or_its_repo() {
    let shown: Vec<String> = [
        Owner::You,
        Owner::Plugin("figma".to_owned()),
        Owner::Repo("app".to_owned()),
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(shown, ["you", "figma", "repo app"]);
}

// ---- links (SK-36) --------------------------------------------------------------------------

/// What a person is shown for a folder purlis left out as a link.
fn link() -> String {
    Reason::NotASkill(Missing::Link).to_string()
}

#[cfg(unix)]
#[test]
fn a_linked_skill_folder_on_any_level_is_left_out_saying_purlis_does_not_read_it() {
    let plane = Plane::new();
    let outside = plane._tmp.path().join("outside");
    skill(&outside, "secret", &whole("secret"));
    for dir in [
        plane.project(),
        plane.project_local(),
        plane.workspace("smart-ide"),
        plane.workspace_local("smart-ide"),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), dir.join("linked")).unwrap();
    }

    let set = plane.set(Some("smart-ide"));

    assert!(set.effective.is_empty(), "{:?}", set.effective);
    assert_eq!(
        left_out(&set),
        [
            ("linked", Origin::Project, false, link()),
            ("linked", Origin::Project, true, link()),
            ("linked", Origin::Workspace, false, link()),
            ("linked", Origin::Workspace, true, link()),
        ]
    );
    assert_eq!(link(), "a link, so purlis doesn't read it");
}

#[cfg(unix)]
#[test]
fn a_workspace_or_persona_folder_that_is_a_link_is_never_read_through() {
    let plane = Plane::new();
    let outside = plane._tmp.path().join("outside");
    for dir in [PROJECT_DIR, LOCAL_DIR] {
        skill(&outside.join(dir), "secret", &whole("secret"));
    }
    std::os::unix::fs::symlink(&outside, plane.root.join("workspaces/linked")).unwrap();
    std::fs::create_dir_all(plane.root.join("personas")).unwrap();
    std::os::unix::fs::symlink(&outside, plane.root.join("personas/linked")).unwrap();

    let set = plane.as_persona(Some("linked"), "linked", &[], None, None);

    assert!(set.effective.is_empty(), "{:?}", set.effective);
    let reasons: Vec<(Origin, String)> = set
        .left_out
        .iter()
        .map(|one| (one.origin, one.reason.to_string()))
        .collect();
    assert_eq!(reasons.len(), 4, "{reasons:?}");
    assert!(
        reasons.iter().all(|(origin, why)| {
            matches!(origin, Origin::Persona | Origin::Workspace) && *why == link()
        }),
        "{reasons:?}"
    );
}

#[cfg(unix)]
#[test]
fn share_of_a_persona_or_workspace_whose_folder_is_a_link_moves_nothing() {
    // A share is a move, and a level's folder that is a link would move what it leads to.
    let plane = Plane::new();
    let outside = plane._tmp.path().join("outside");
    skill(&outside.join(LOCAL_DIR), "secret", &whole("secret"));
    std::fs::create_dir_all(plane.root.join("workspaces")).unwrap();
    std::os::unix::fs::symlink(&outside, plane.root.join("workspaces/linked")).unwrap();
    std::fs::create_dir_all(plane.root.join("personas")).unwrap();
    std::os::unix::fs::symlink(&outside, plane.root.join("personas/linked")).unwrap();

    for at in [
        Whose::Workspace("linked".into()),
        Whose::Persona("linked".into()),
    ] {
        assert!(share(&plane.root, &at, "secret").is_err(), "{at:?}");
    }
    assert!(outside.join(LOCAL_DIR).join("secret/SKILL.md").is_file());
    assert!(!outside.join(PROJECT_DIR).exists());
}

#[cfg(unix)]
#[test]
fn share_into_a_skills_folder_that_is_a_link_moves_nothing_at_the_root_or_in_a_workspace() {
    // A chat could make a level's `skills/` a link to a folder its sandbox denies it; the app,
    // which is not sandboxed, must not move a skill the chat wrote into it.
    let plane = Plane::new();
    let denied = plane._tmp.path().join("denied");
    std::fs::create_dir_all(&denied).unwrap();
    skill(&plane.project_local(), "notes", &whole("notes"));
    skill(
        &plane.workspace_local("smart-ide"),
        "notes",
        &whole("notes"),
    );
    std::os::unix::fs::symlink(&denied, plane.project()).unwrap();
    std::os::unix::fs::symlink(&denied, plane.workspace("smart-ide")).unwrap();

    for at in [Whose::Project, Whose::Workspace("smart-ide".into())] {
        let refused = share(&plane.root, &at, "notes").unwrap_err();
        assert!(
            refused.contains("so nothing was moved"),
            "{at:?}: {refused}"
        );
    }
    assert_eq!(std::fs::read_dir(&denied).unwrap().count(), 0);
    assert!(plane.project_local().join("notes/SKILL.md").is_file());
    assert!(
        plane
            .workspace_local("smart-ide")
            .join("notes/SKILL.md")
            .is_file()
    );
}

#[cfg(unix)]
#[test]
fn share_of_a_skill_whose_toml_is_a_link_moves_nothing() {
    let plane = Plane::new();
    let outside = plane._tmp.path().join("outside.toml");
    std::fs::write(&outside, "grant = \"all\"\n").unwrap();
    skill(&plane.project_local(), "notes", &whole("notes"));
    std::os::unix::fs::symlink(&outside, plane.project_local().join("notes.toml")).unwrap();

    let refused = share(&plane.root, &Whose::Project, "notes").unwrap_err();

    assert!(refused.contains("so nothing was moved"), "{refused}");
    assert!(plane.project_local().join("notes/SKILL.md").is_file());
    assert!(
        plane
            .project_local()
            .join("notes.toml")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!plane.project().join("notes").exists());
    assert!(
        plane
            .project()
            .join("notes.toml")
            .symlink_metadata()
            .is_err()
    );
}

#[test]
fn a_repo_skills_folder_purlis_could_not_list_takes_no_name_and_leaves_out_every_plane_skill_a_repo_copy_could_hide()
 {
    // SK-44 (#3133): a clone's skills folder purlis did not list whole may hold a copy of any
    // plane skill. Where a repo copy wins, every plane skill is left out, and the folder's row
    // is never a skill the chat gets nor a name it takes.
    let plane = Plane::new();
    skill(&plane.workspace("smart-ide"), "review", &whole("review"));
    skill(&plane.workspace("smart-ide"), "skills", &whole("skills"));
    let crowded = || OutsideSkill {
        unread: Some(Unread::Crowded),
        ..outside("skills", Origin::Repo, repo("api"), false)
    };

    for route in [Route::Config, Route::Briefing] {
        let set = plane
            .set(Some("smart-ide"))
            .with_outside(route, [crowded()]);

        assert_eq!(
            effective(&set),
            Vec::<(&str, Origin, bool)>::new(),
            "{route:?}"
        );
        let why = "purlis could not read the skills folder of api whole, so a copy of this \
                   skill may be there, and a chat in that repo would get that one"
            .to_owned();
        assert_eq!(
            left_out(&set),
            [
                ("review", Origin::Workspace, false, why.clone()),
                ("skills", Origin::Workspace, false, why),
            ],
            "{route:?}"
        );
        assert_eq!(set.clashes, [], "{route:?}");
    }

    // On the plugin route both load, so nothing is left out, and the row is still no skill.
    let set = plane
        .set(Some("smart-ide"))
        .with_outside(Route::Plugin, [crowded()]);
    assert_eq!(set.from_the_plane().count(), 2);
    assert!(set.effective.iter().all(|e| e.origin != Origin::Repo));
}
