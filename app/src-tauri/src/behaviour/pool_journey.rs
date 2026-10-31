// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The whole way a person goes through a Docker pool, in one test** (#2041's runners): the
//! pool path had never run end to end with a real profile, so each bug of it reached the person
//! first. Here a pool is added from the runner image as it ships, with no harness in it; a
//! profile with a settings folder as large as a real one goes to its runner; the runner fetches
//! Claude Code from its vendor's release; a chat runs on it; a chat moves back and there again;
//! the container is removed outside purlis and the runner made again; an update is followed.
//! Each step checks what the window would show.
//!
//! Two things stand in, and only these. **The sign-in**: no account signs in on CI, so the
//! profile's folder on the runner is given a credentials file of a token no service would take,
//! as Claude Code's own login writes it, and Claude Code itself says it is signed in there. No
//! model answers. **The image's name**: an update is followed only between images of purlis's
//! own name, and this build's is in the job's registry, so the `docker` this test's pool uses
//! is the real one, told the job's registry wherever a command names purlis's, and naming
//! purlis's again in what it answers. The digests are real.

use std::cell::RefCell;

use purlis_core::profilestore::onrunner::{self, Kept, LeftBehind};
use purlis_core::profilestore::sign_in::Standing;
use purlis_core::profilestore::{self, Folders, MachineProfile};

use super::*;
use crate::host::{Moving, Named};
use crate::runnerlinks::{Machine, ToARunner};

/// The variable naming the digest of the runner image as it ships, and of the one an update
/// moves to, both in the parity job's registry (`.github/workflows/ci.yml`).
const IMAGE: &str = "DOCKER_POOL_JOURNEY_IMAGE";
const NEXT_IMAGE: &str = "DOCKER_POOL_JOURNEY_NEXT_IMAGE";

/// Where the parity job keeps them.
const REGISTRY_NAME: &str = "localhost:5000/purlis-runner";

/// The Claude Code version purlis supports, which the profile is pinned to.
const SUPPORTED: &str = include_str!("../../../../tests/fixtures/harness-versions/claude.txt");

/// A fetch from the vendor's release, and a large settings folder over a link, take a while.
const LONG: Duration = Duration::from_secs(300);

const PROFILE: &str = "work";
const WORKSPACE: &str = "app";

/// How many skills the profile's settings hold, each of three files, and how large each one's
/// data is: about 40 MiB in 4,500 files, as a person's folder of skills is.
const SKILLS: usize = 1500;
const SKILL_DATA_BYTES: usize = 26 * 1024;

/// The `docker` the pool uses: the real one, told the job's registry in place of purlis's name.
fn a_docker_of_the_jobs_registry(folder: &Path) -> purlis_core::dockerpool::Docker {
    use std::os::unix::fs::PermissionsExt as _;
    let path = purlis_core::envvar::var("PATH");
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let real = purlis_core::dockerpool::Docker::on_this_machine(path.as_deref(), home.as_deref())
        .expect("docker is on this machine");
    let script = folder.join("docker");
    let own = purlis_core::dockerpool::PURLIS_RUNNER_IMAGE.replace('/', "\\/");
    let registry = REGISTRY_NAME.replace('/', "\\/");
    let (sed_own, sed_registry) = (own.replace('.', "\\."), registry.replace('.', "\\."));
    // What it answers names purlis's image again, as a pool checks the image its container runs.
    // Never a stream: `exec` and `attach` carry the link between hosts as they are.
    std::fs::write(
        &script,
        format!(
            "#!/bin/bash\n\
             args=()\n\
             stream=\n\
             for word in \"$@\"; do\n  \
               case \"$word\" in exec|attach) stream=yes ;; esac\n  \
               args+=(\"${{word//{own}/{registry}}}\")\n\
             done\n\
             [ -n \"$stream\" ] && exec '{real}' \"${{args[@]}}\"\n\
             '{real}' \"${{args[@]}}\" | sed 's/{sed_registry}/{sed_own}/g'\n\
             exit \"${{PIPESTATUS[0]}}\"\n",
            real = real.program().display(),
        ),
    )
    .expect("its script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("it runs");
    purlis_core::dockerpool::Docker::at(script.into_os_string())
}

/// Bytes that do not compress, different for each `seed`.
fn data(seed: usize, size: usize) -> Vec<u8> {
    use sha2::{Digest as _, Sha256};
    let mut bytes = Vec::with_capacity(size + 32);
    let mut block = Sha256::digest(seed.to_le_bytes()).to_vec();
    while bytes.len() < size {
        bytes.extend_from_slice(&block);
        block = Sha256::digest(&block).to_vec();
    }
    bytes.truncate(size);
    bytes
}

/// A settings folder as large as a real one, with what never travels inside it.
fn a_large_settings_folder(folder: &Path) {
    let write = |path: &str, bytes: &[u8]| {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().expect("under the folder")).expect("its folder");
        std::fs::write(path, bytes).expect("it is written");
    };
    write("CLAUDE.md", b"# Be kind.\n");
    write(
        "settings.json",
        b"{\"permissions\":{\"allow\":[\"Read\"]}}\n",
    );
    for skill in 0..SKILLS {
        let name = format!("skills/skill-{skill:04}");
        write(
            &format!("{name}/SKILL.md"),
            format!("---\nname: skill-{skill:04}\ndescription: Skill {skill}.\n---\n").as_bytes(),
        );
        write(&format!("{name}/reference.md"), b"How it is done.\n");
        write(&format!("{name}/data.bin"), &data(skill, SKILL_DATA_BYTES));
    }
    // What a skill has no use for on a runner, large enough to be noticed if it went.
    write("skills/skill-0000/node_modules/left/index.js", b"left\n");
    write(
        "skills/skill-0000/node_modules/left/big.bin",
        &data(SKILLS, 16 << 20),
    );
    write("skills/skill-0001/.git/HEAD", b"ref: refs/heads/main\n");
    write(".git/HEAD", b"ref: refs/heads/main\n");
}

/// The desktop's profile: its settings `folder`, pinned to the version purlis supports, in this
/// machine's store as the window makes one and in the project as a chat's move finds it.
fn the_desktops_profile(machine: &Machine, root: &Path, folder: &Path) {
    let version = SUPPORTED.split(' ').next().expect("a version").trim();
    profilestore::update(&machine.config, |store| {
        store.profiles.push(MachineProfile {
            name: PROFILE.to_owned(),
            harness: "claude".to_owned(),
            title: PROFILE.to_owned(),
            folders: Folders {
                config: folder.to_path_buf(),
                data: None,
            },
            adopted: None,
            imported: None,
            sign_in: None,
            pinned: Some(profilestore::pin::Pinned {
                version: version.to_owned(),
                at: 1,
            }),
            sandbox_only: false,
            kept_for: None,
            rest: Default::default(),
        });
    })
    .expect("the profile is kept");
    let persona = root.join("personas").join("steward");
    std::fs::create_dir_all(&persona).expect("its persona");
    std::fs::write(persona.join("persona.md"), "# steward\n").expect("its persona");
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.{PROFILE}]\nkind = \"claude\"\ncommand = [\"claude\"]\nenv = {{ CLAUDE_CONFIG_DIR = {:?} }}\n",
            folder.display().to_string()
        ),
    )
    .expect("its profile");
    let declared = purlis_core::profiles::current(root);
    purlis_core::profiletrust::record_launched(
        root,
        PROFILE,
        &purlis_core::profiletrust::fingerprint(declared.get(PROFILE).expect("declared")),
    )
    .expect("approved");
}

/// The pool's runner now: an update and Make the runner again change it.
fn the_runner(machine: &Machine, pool: &str) -> OnTheRunner {
    let kept = purlis_core::runners::runner_for(&machine.config, pool).expect("its runner is kept");
    OnTheRunner {
        docker: machine.docker.clone(),
        container: kept.made.container.as_str().to_owned(),
        pool: pool.to_owned(),
    }
}

/// Keeps `kept` on pool `pool`'s runner over a link of its own, as a start and a pool's row do
/// (`crate::poolprofiles::Linked`, whose machine has this build's keyring): how its harness
/// stands there, and what of its settings stayed behind.
fn keep(
    machine: &Machine,
    pool: &str,
    kept: &Kept,
    behind: &RefCell<Vec<LeftBehind>>,
) -> Result<Option<Standing>, String> {
    let runner =
        purlis_core::runners::runner_for(&machine.config, pool).map_err(|why| why.to_string())?;
    let to = ToARunner::of_kept_waking(machine, &runner)?;
    let Some(said) = crate::runnerlinks::keep_profiles(&to, vec![kept.clone()])? else {
        return Ok(None);
    };
    said.into_iter()
        .next()
        .unwrap_or_else(|| Err("the runner did not answer".to_owned()))
        .map(|kept| {
            behind.replace(kept.left_behind);
            Some(onrunner::standing_on_the_wire(kept.standing))
        })
}

/// What a start on the pool asks first (`Chats`'s start, through `Linked`).
fn ready_to_start(machine: &Machine, root: &Path, pool: &str) -> Result<(), String> {
    onrunner::ready_on(&machine.config, root, pool, PROFILE, &|kept| {
        keep(machine, pool, kept, &RefCell::default())
    })
}

/// Waits until the pool's row says `wanted` of the profile, as the window asks again while the
/// runner fetches its harness (#4008). What stayed behind of its settings is `behind`.
fn the_row_says(machine: &Machine, pool: &str, wanted: &Standing) -> Vec<LeftBehind> {
    let record = profilestore::list(&machine.config)
        .expect("the store reads")
        .into_iter()
        .find(|record| record.name == PROFILE)
        .expect("the profile");
    let kept = Kept::of(&record)
        .expect("it goes to a runner")
        .with_settings_of(&record);
    let deadline = Instant::now() + LONG;
    loop {
        let behind = RefCell::default();
        let said = keep(machine, pool, &kept, &behind)
            .unwrap_or_else(|why| panic!("the runner did not keep the profile: {why}"))
            .expect("this runner keeps profiles");
        if said == *wanted {
            return behind.into_inner();
        }
        assert!(
            Instant::now() < deadline,
            "the row says {said:?}, and never {wanted:?}"
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The profile's settings folder on `runner`, found where its settings went.
fn the_settings_on(runner: &OnTheRunner) -> String {
    let skill = runner.prints("find \"$HOME\" -path '*/skills/skill-0000/SKILL.md' -print -quit");
    let skill = skill.trim();
    assert!(!skill.is_empty(), "the runner has none of the skills");
    skill
        .strip_suffix("/skills/skill-0000/SKILL.md")
        .expect("found by that path")
        .to_owned()
}

/// Every skill reached `runner` whole, and what never travels did not.
fn the_settings_went_whole_to(runner: &OnTheRunner, folder: &Path) {
    let there = the_settings_on(runner);
    let count = |name: &str| {
        runner
            .prints(&format!("find '{there}' -name '{name}' | wc -l"))
            .trim()
            .to_owned()
    };
    assert_eq!(count("SKILL.md"), SKILLS.to_string(), "every skill went");
    assert_eq!(count("node_modules"), "0", "a skill's node_modules stays");
    assert_eq!(count(".git"), "0", "a .git stays");
    assert_eq!(
        runner
            .prints(&format!("wc -c < '{there}/skills/skill-1499/data.bin'"))
            .trim(),
        SKILL_DATA_BYTES.to_string(),
    );
    assert!(folder.join("skills/skill-0000/node_modules").exists());
}

/// The test's sign-in on `runner`: a credentials file as Claude Code's own login writes it,
/// of a token no service would take. See the module's words.
fn signed_in_on(runner: &OnTheRunner) {
    let there = the_settings_on(runner);
    runner.runs(
        false,
        &format!("umask 077 && cat > '{there}/.credentials.json'"),
        br#"{"claudeAiOauth":{"accessToken":"parity-not-a-token","refreshToken":"parity-not-a-token","expiresAt":4102444800000,"scopes":["user:inference","user:profile"],"subscriptionType":"max"}}"#,
    );
}

fn a_chat(first_message: Option<&str>, resume: Option<String>) -> Named {
    Named {
        workspace: Some(WORKSPACE.to_owned()),
        first_message: first_message.map(str::to_owned),
        resume,
        ..named("", false)
    }
}

/// A chat started as the window starts one on a pool: its profile readied, then started, and
/// real Claude Code drawing in its pane.
fn a_chat_runs(target: &Target, machine: &Machine, pool: &str, chat: &Named) -> (u32, Pane) {
    ready_to_start(machine, &target.root, pool)
        .unwrap_or_else(|why| panic!("the chat is not ready to start: {why}"));
    let id = target
        .sessions
        .open_named(None, chat, Hearing::default(), &|_| {})
        .unwrap_or_else(|why| panic!("the chat starts on the runner: {why}"))
        .number;
    let pane = Pane::on(target, id);
    pane.shows_within("Claude Code", LONG);
    (id, pane)
}

fn in_the_journey(
    target: &Target,
    machine: &Machine,
    pool: &str,
    next: &purlis_core::dockerpool::Image,
) {
    // Add a pool from the runner image: its row is there, and nothing says it was removed.
    let shown = crate::dockerpools::pools_on(&machine.config).expect("the pools read");
    assert_eq!(
        shown.pools.iter().map(|row| &row.name).collect::<Vec<_>>(),
        [pool],
    );
    assert!(
        crate::dockerpools::removed_on(machine)
            .expect("asked")
            .is_empty()
    );
    let runner = the_runner(machine, pool);
    assert_eq!(
        runner.prints("command -v claude || echo none").trim(),
        "none",
        "the runner image ships no harness"
    );

    // Keep a profile with a large settings folder: all of it travels, and nothing is left behind.
    let settings = tempfile::tempdir().expect("the profile's folder");
    let settings = settings.path().canonicalize().expect("its folder");
    a_large_settings_folder(&settings);
    the_desktops_profile(machine, &target.root, &settings);
    let clone = target.root.join("workspaces").join(WORKSPACE).join("web");
    std::fs::create_dir_all(&clone).expect("its clone");
    std::fs::write(clone.join("README.md"), "# web\n").expect("its file");
    a_repository(&clone);

    // Fetch the harness: asked of its row, the runner fetches it from the vendor's release, and
    // Claude Code itself then says it is signed out there.
    let behind = the_row_says(machine, pool, &Standing::SignedOut);
    assert_eq!(behind, Vec::new(), "nothing of the settings stays behind");
    the_settings_went_whole_to(&runner, &settings);

    // A start while signed out starts nothing, and says what to do.
    let refused = ready_to_start(machine, &target.root, pool)
        .expect_err("a signed-out profile starts nothing");
    assert!(refused.contains("is not signed in on pool"), "{refused}");
    assert!(refused.contains("Sign in on pool"), "{refused}");

    // Start a real Claude Code chat.
    signed_in_on(&runner);
    the_row_says(machine, pool, &Standing::SignedIn);
    let (id, pane) = a_chat_runs(target, machine, pool, &a_chat(Some("Say hello."), None));
    pane.shows_within("Say hello.", LONG);
    let there = the_settings_on(&runner);
    let conversation = {
        let deadline = Instant::now() + LONG;
        loop {
            let found = runner.prints(&format!(
                "find '{there}/projects' -name '*.jsonl' 2>/dev/null | head -n 1"
            ));
            if let Some(stem) = Path::new(found.trim()).file_stem() {
                break stem.to_string_lossy().into_owned();
            }
            assert!(Instant::now() < deadline, "the chat kept no conversation");
            std::thread::sleep(Duration::from_secs(1));
        }
    };
    target.sessions.close(id).expect("it closes");

    // Move it back here: its conversation and the work done there come with it.
    let on_the_runner = format!(
        "{}/workspaces/{WORKSPACE}/web",
        target.project_on_its_runner(&runner)
    );
    runner.runs(
        false,
        &format!("printf 'made there\\n' > '{on_the_runner}/there.txt'"),
        b"",
    );
    let moving = [Moving {
        workspace: WORKSPACE.to_owned(),
        profile: PROFILE.to_owned(),
        conversation: conversation.clone(),
    }];
    target
        .sessions
        .move_out(pool, &moving)
        .unwrap_or_else(|why| panic!("the chat moves back: {why}"));
    assert_eq!(
        std::fs::read_to_string(clone.join("there.txt")).expect("the work came back"),
        "made there\n"
    );
    let here = settings.join("projects");
    assert!(
        walkdir_finds(&here, &format!("{conversation}.jsonl")),
        "its conversation came back into {}",
        here.display()
    );

    // And there again: what was done here goes with it, and the chat goes on where it was.
    std::fs::write(clone.join("here.txt"), "made here\n").expect("its file");
    onrunner::ready_to_move_on(&machine.config, pool, PROFILE, &|kept| {
        keep(machine, pool, kept, &RefCell::default())
    })
    .unwrap_or_else(|why| panic!("the profile is ready for the move: {}", why.said()));
    target
        .sessions
        .move_in(pool, &moving)
        .unwrap_or_else(|why| panic!("the chat moves there: {why}"));
    assert_eq!(
        runner.prints(&format!("cat '{on_the_runner}/here.txt'")),
        "made here\n"
    );
    let (id, _pane) = a_chat_runs(
        target,
        machine,
        pool,
        &a_chat(None, Some(conversation.clone())),
    );
    target.sessions.close(id).expect("it closes");

    // Delete the container outside purlis: the pool's row says so, a start says what to do,
    // and it is made again with its sign-ins and its settings.
    let out =
        purlis_core::forklock::output(std::process::Command::new(machine.docker.program()).args([
            "rm",
            "--force",
            runner.container.as_str(),
        ]))
        .expect("docker runs");
    assert!(out.status.success(), "removed outside purlis: {out:?}");
    let removed = crate::dockerpools::removed_on(machine).expect("asked");
    assert_eq!(
        removed,
        [crate::dockerpools::PoolRemoved {
            name: pool.to_owned(),
            home_kept: true,
        }],
    );
    let refused =
        ready_to_start(machine, &target.root, pool).expect_err("nothing starts on it now");
    assert!(refused.contains("removed outside purlis"), "{refused}");

    let again = crate::dockerpools::make_again_on(machine, pool)
        .unwrap_or_else(|why| panic!("the runner is made again: {why}"));
    assert!(
        again.sign_ins_kept,
        "its home was there, so its sign-ins came"
    );
    assert!(
        crate::dockerpools::removed_on(machine)
            .expect("asked")
            .is_empty()
    );
    let runner = the_runner(machine, pool);
    the_row_says(machine, pool, &Standing::SignedIn);
    the_settings_went_whole_to(&runner, &settings);
    let (old_chat, old_pane) = a_chat_runs(target, machine, pool, &a_chat(None, None));

    // Follow an update: new chats start on a runner of the new image, the old one keeps its
    // chat until it ends, and then it is removed and its sign-ins follow.
    let chats_on = |to: &ToARunner| crate::runnerlinks::chats_on(to);
    let said = crate::poolfollow::follow_and_drain(machine, Some(next), &chats_on);
    assert!(
        said.iter()
            .any(|line| line.contains("starts its new chats on runner")),
        "{said:?}"
    );
    assert_eq!(crate::poolfollow::waiting_on(&machine.config, pool), 1);
    let followed = the_runner(machine, pool);
    assert_ne!(followed.container, runner.container, "a runner of its own");
    assert_eq!(
        crate::dockerpools::pools_on(&machine.config)
            .expect("the pools read")
            .pools[0]
            .image,
        next.as_str(),
    );
    old_pane.shows("Claude Code");
    let refused = ready_to_start(machine, &target.root, pool)
        .expect_err("a sign-in follows only once the old runner has drained");
    assert!(refused.contains("is not signed in on pool"), "{refused}");

    target.sessions.close(old_chat).expect("it closes");
    let deadline = Instant::now() + LONG;
    let said = loop {
        let said = crate::poolfollow::follow_and_drain(machine, None, &chats_on);
        if said.iter().any(|line| line.contains("had no chat left")) {
            break said;
        }
        assert!(Instant::now() < deadline, "it never drained: {said:?}");
        std::thread::sleep(Duration::from_secs(1));
    };
    assert_eq!(
        crate::poolfollow::waiting_on(&machine.config, pool),
        0,
        "{said:?}"
    );
    the_row_says(machine, pool, &Standing::SignedIn);
    let (id, _pane) = a_chat_runs(target, machine, pool, &a_chat(None, None));
    target.sessions.close(id).expect("it closes");
}

/// Whether `file` is anywhere under `folder`.
fn walkdir_finds(folder: &Path, file: &str) -> bool {
    std::fs::read_dir(folder).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry.file_name() == file
                || (entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && walkdir_finds(&entry.path(), file))
        })
    })
}

/// Run by the parity job, which names both images; elsewhere it runs nothing.
#[test]
fn in_a_docker_pool() {
    use purlis_core::dockerpool::{Image, PURLIS_RUNNER_IMAGE};
    let (Ok(image), Ok(next)) = (std::env::var(IMAGE), std::env::var(NEXT_IMAGE)) else {
        assert!(
            std::env::var_os(PARITY).is_none(),
            "{PARITY} is set and {IMAGE} names no runner image: the parity job built none"
        );
        return;
    };
    let scripts = tempfile::tempdir().expect("a folder for its docker");
    let docker = a_docker_of_the_jobs_registry(scripts.path());
    let (target, machine, pool) =
        a_window_on_a_pool_of(&format!("{PURLIS_RUNNER_IMAGE}@{image}"), docker);
    let next = Image::pinned(&format!("{PURLIS_RUNNER_IMAGE}@{next}")).expect("named by digest");
    in_the_journey(&target, &machine, &pool, &next);
}
