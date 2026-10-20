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
}

impl Plane {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("plane");
        for ws in ["smart-ide", "ops"] {
            std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
        }
        Self { _tmp: tmp, root }
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

    fn manifest(&self, ws: &str, json: &str) {
        std::fs::write(
            self.root.join("workspaces").join(ws).join("workspace.json"),
            json,
        )
        .unwrap();
    }

    /// The set of a chat in `workspace` (`None`: at the plane root), with git keeping every
    /// local skill on this machine.
    fn set(&self, workspace: Option<&str>) -> SkillSet {
        self.set_with(workspace, &|_| Ok(()))
    }

    fn set_with(&self, workspace: Option<&str>, kept: &KeptLocal<'_>) -> SkillSet {
        skill_set(
            Some(&self.root),
            &Machine {
                built_in: None,
                kept_local: Some(kept),
            },
            &Chat {
                workspace,
                persona: None,
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
        "not granted to this workspace: its grant in deploy.toml cannot be read (grant is \
         \"everyone\", which is none of \"root\", \"all\" or a list of workspace names), so \
         it is for chats at the project's root only"
    );
    assert!(
        reasons[1].starts_with(
            "not granted to this workspace: its grant in lint.toml cannot be read (it is not TOML"
        ),
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
    plane.manifest(
        "smart-ide",
        r#"{"name": "smart-ide", "settings": {"skills": {"off": ["deploy"]}}}"#,
    );

    let set = plane.set(Some("smart-ide"));

    assert_eq!(effective(&set), [("lint", Origin::Project, false)]);
    assert_eq!(
        left_out(&set),
        [(
            "deploy",
            Origin::Project,
            false,
            "turned off in this workspace (settings.skills.off in its workspace.json)".to_owned()
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
    plane.manifest(
        "smart-ide",
        r#"{"settings": {"skills": {"off": ["deploy"], "on": ["deploy"]}}}"#,
    );

    let set = plane.set(Some("smart-ide"));

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [("deploy", Origin::Project, false, NOT_GRANTED.to_owned())]
    );
    assert_eq!(turned_off(&plane.root, "smart-ide"), ["deploy"]);
}

#[test]
fn a_turn_off_list_that_is_not_a_list_of_names_turns_nothing_off() {
    let plane = Plane::new();
    skill(&plane.project(), "deploy", &whole("deploy"));
    plane.grant("deploy", "grant = \"all\"\n");
    plane.manifest(
        "smart-ide",
        r#"{"settings": {"skills": {"off": "deploy"}}}"#,
    );
    plane.manifest("ops", "not json");

    assert_eq!(effective(&plane.set(Some("smart-ide"))).len(), 1);
    assert_eq!(effective(&plane.set(Some("ops"))).len(), 1);
    assert!(turned_off(&plane.root, "smart-ide").is_empty());
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
    plane.manifest(
        "smart-ide",
        r#"{"settings": {"skills": {"off": ["review"]}}}"#,
    );

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
        Err("git would commit skills.local/scratch/SKILL.md".to_owned())
    };

    let set = plane.set_with(None, &kept);

    assert!(set.effective.is_empty());
    assert_eq!(
        left_out(&set),
        [(
            "scratch",
            Origin::Project,
            true,
            "a local skill is read only while git keeps it on this machine, and git would \
             commit skills.local/scratch/SKILL.md: add /skills.local/ to the plane's .gitignore"
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
            .contains("purlis could not ask git whether it is ignored"),
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
        },
        &Chat {
            workspace: Some("smart-ide"),
            persona: None,
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

    let moved = share(&plane.root, None, "scratch").unwrap();

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

    let moved = share(&plane.root, Some("smart-ide"), "notes").unwrap();

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

    let refused = share(&plane.root, None, "deploy").unwrap_err();

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
        share(&plane.root, None, "deploy").unwrap_err(),
        "no local skill is named \"deploy\" at the project's root"
    );
    assert_eq!(
        share(&plane.root, Some("smart-ide"), "deploy").unwrap_err(),
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
