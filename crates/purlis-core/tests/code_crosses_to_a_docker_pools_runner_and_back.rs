// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Code crosses to a Docker pool's runner and back, for real (#2046): a desktop's branch, with
//! its uncommitted work, is sent over the link through `docker exec` into the project the runner
//! keeps in `<data>/projects/<project id>`, checked out there; a commit the runner makes comes
//! back to the desktop under `refs/remotes/<pool>/` alone; and the runner holds no forge
//! credential. Everything it made is removed, whatever the outcome.
//!
//! It needs Docker and a runner image of this build, as `a_docker_pool_runner_is_linked_through_
//! docker_exec` does: named by digest in `DOCKER_POOL_TEST_IMAGE`, with `docker` on `PATH` or
//! the command `DOCKER_POOL_TEST_DOCKER` names. Without the image it runs nothing and passes.
//!
//! The desktop's repository keeps its git directory beside its work tree, so this test runs
//! where a sandbox refuses a folder's own `.git/config`.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use purlis_core::codelink::{self, Bundled, Ref};
use purlis_core::dockerpool::{Docker, Image, Made, Runner};
use purlis_host::CommandConnector;
use purlis_session_protocol::hostlink::{
    self, Handshake, Identity, LinkKey, Linked, Liveness, Pinned,
};
use purlis_session_protocol::link::Acceptor;
use purlis_session_protocol::session::{Client, CodeRef};
use purlis_session_protocol::view::{Opened, Viewer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PATIENCE: Duration = Duration::from_secs(60);
const PROJECT: &str = "01J9PRJCT00000000000000000";
const REPO: &str = "workspaces/app/web";

/// Removes what the test made, however it ends.
struct Removes(Docker, Option<Made>);

impl Drop for Removes {
    fn drop(&mut self) {
        if let Some(made) = self.1.take() {
            let _ = self.0.remove(&made);
        }
    }
}

/// git for the desktop's own fixture, its git directory beside its work tree.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = purlis_core::forklock::output(
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "core.excludesFile=/dev/null",
            ])
            .args(args),
    )
    .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A command in the runner's container, as its own user, its output as text.
fn in_the_runner(docker: &Docker, made: &Made, script: &str) -> String {
    let out = purlis_core::forklock::output(Command::new(docker.program()).args([
        "exec",
        made.container.as_str(),
        "sh",
        "-c",
        script,
    ]))
    .expect("docker runs");
    assert!(out.status.success(), "{script}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

async fn linked(
    docker: &Docker,
    made: &Made,
    ours: Identity,
    runner: Pinned,
) -> (Client, Acceptor) {
    let connector = CommandConnector::new(docker.program(), made.connect_args(), {
        let (docker, made) = (docker.clone(), made.clone());
        Arc::new(move || docker.check(&made).map_err(|why| why.to_string()))
    })
    .preparing({
        let docker = docker.clone();
        Arc::new(move || {
            let (env, cleared, held) = docker
                .prepared()
                .map_err(|why| why.to_string())?
                .into_parts();
            Ok(purlis_host::Prepared { env, cleared, held })
        })
    });
    let mut links = hostlink::keep_linked(
        connector,
        Handshake::THIS_BUILD,
        ours,
        runner,
        Liveness::default(),
    );
    match tokio::time::timeout(PATIENCE, links.next()).await {
        Ok(Some(Linked::Up(link))) => {
            let (client, streams, _events) = Client::new(*link);
            tokio::spawn(async move { while links.next().await.is_some() {} });
            (client, streams)
        }
        other => panic!("the paired desktop is not linked: {:?}", other.is_ok()),
    }
}

async fn code_stream(streams: &mut Acceptor) -> purlis_session_protocol::link::Stream {
    let stream = tokio::time::timeout(PATIENCE, streams.accept())
        .await
        .expect("the runner opened a code stream")
        .unwrap();
    match Viewer::default().accept_any(stream).await.unwrap() {
        Opened::Code(_, stream) => stream,
        Opened::View(_) => panic!("a view, where a code stream was meant"),
    }
}

fn wire(refs: &[Ref]) -> Vec<CodeRef> {
    refs.iter()
        .map(|one| CodeRef {
            name: one.name.clone(),
            oid: one.oid.clone(),
        })
        .collect()
}

fn core(refs: Vec<CodeRef>) -> Vec<Ref> {
    refs.into_iter()
        .map(|one| Ref {
            name: one.name,
            oid: one.oid,
        })
        .collect()
}

#[test]
fn a_branch_and_its_uncommitted_work_reach_a_pools_runner_and_its_commits_come_back() {
    purlis_core::unsteered!();
    let Ok(image) = std::env::var("DOCKER_POOL_TEST_IMAGE") else {
        return;
    };
    let settings = tempfile::tempdir().unwrap();
    let docker = Docker::at(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()))
        .with_settings_in(settings.path().join("docker-settings"));
    let image = Image::pinned(&image).expect("the image is named by digest");
    let (user, home) = docker
        .declared(&image)
        .expect("the image declares its user");
    let name = format!("test-{}", ulid::Ulid::generate().to_string().to_lowercase());
    let runner = Runner::new(&name, image, user, &home).unwrap();
    let mut removes = Removes(docker.clone(), None);
    let made = docker
        .make(&runner)
        .expect("the runner's container is made");
    removes.1 = Some(made.clone());
    let ours = Identity {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap(),
    };
    let pinned = Pinned {
        device: ours.device.clone(),
        key: ours.key.public(),
    };
    let runner_pinned = docker.pair(&made, &pinned).expect("the runner is paired");

    // The desktop: a clone on `fix-login`, with work not committed yet.
    let top = tempfile::tempdir().unwrap();
    let top = top.path().canonicalize().unwrap();
    // Outside any project's workspaces, where purlis reads a git directory kept beside its tree.
    let desk = top.join("desktop/web");
    std::fs::create_dir_all(&desk).unwrap();
    git(
        &desk,
        &[
            "init",
            "-q",
            "-b",
            "main",
            &format!("--separate-git-dir={}", top.join("web.gitdir").display()),
            ".",
        ],
    );
    std::fs::write(desk.join("README"), "web\n").unwrap();
    git(&desk, &["add", "README"]);
    git(&desk, &["commit", "-q", "-m", "one"]);
    git(&desk, &["checkout", "-q", "-b", "fix-login"]);
    std::fs::write(desk.join("login.rs"), "fn log_in() {}\n").unwrap();
    let fix_here = git(&desk, &["rev-parse", "fix-login"]);
    let scratch: PathBuf = top.join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let into = scratch.join("send.bundle");
    let sent = codelink::bundle_branch(
        &desk,
        Some((desk.as_path(), &[][..])),
        "fix-login",
        &[],
        &into,
        &scratch,
    )
    .unwrap();
    assert_eq!(sent.bundled, Bundled::Wrote);
    assert_eq!(sent.refs.len(), 2, "the branch and its snapshot");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let back = scratch.join("back.bundle");
    let kept = runtime.block_on(async {
        let (client, mut streams) = linked(&docker, &made, ours, runner_pinned).await;
        client
            .hold_project(PROJECT, "")
            .await
            .expect("the runner holds the project where it keeps it");

        let asked = client.take_code(
            PROJECT,
            REPO,
            1,
            wire(&sent.refs),
            Some("refs/heads/fix-login".into()),
        );
        let writing = async {
            let mut stream = code_stream(&mut streams).await;
            let mut file = tokio::fs::File::open(&into).await.unwrap();
            tokio::io::copy(&mut file, &mut stream).await.unwrap();
            stream.shutdown().await.unwrap();
        };
        let (taken, ()) = tokio::join!(asked, writing);
        let taken = core(taken.expect("the runner took the code"));
        assert!(
            taken
                .iter()
                .any(|one| one.name == "refs/heads/fix-login" && one.oid == fix_here)
        );

        // The project is in the runner's own folder for it, checked out with the work.
        let repo = format!(
            "$(ls -d {}/{}/{PROJECT})/{REPO}",
            purlis_core::datahome::IN_A_RUNNER_CONTAINER,
            purlis_core::datahome::PROJECTS
        );
        assert_eq!(
            in_the_runner(&docker, &made, &format!("cat {repo}/login.rs")),
            "fn log_in() {}"
        );
        assert_eq!(
            in_the_runner(
                &docker,
                &made,
                &format!("git -C {repo} symbolic-ref --short HEAD")
            ),
            "fix-login"
        );
        // A runner chat commits there.
        let on_the_runner = in_the_runner(
            &docker,
            &made,
            &format!(
                "git -C {repo} add -A && git -C {repo} -c user.name=Chat \
                 -c user.email=chat@example.invalid -c commit.gpgsign=false commit -q -m work && \
                 git -C {repo} rev-parse HEAD"
            ),
        );

        let asked = client.give_code(PROJECT, REPO, 2, vec![fix_here.clone()]);
        let reading = async {
            let mut stream = code_stream(&mut streams).await;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            std::fs::write(&back, &bytes).unwrap();
            bytes.len()
        };
        let (given, read) = tokio::join!(asked, reading);
        let heads = core(given.expect("the runner gave its branches"));
        let bundle = (read > 0).then_some(back.as_path());
        let kept = codelink::take_back(&desk, "builds", bundle, &heads).unwrap();

        // The runner holds no forge credential.
        let creds = in_the_runner(
            &docker,
            &made,
            "env | grep -c -E '^(GH_TOKEN|GITHUB_TOKEN|GITLAB_TOKEN|GL_TOKEN)=' ; \
             git config --global --get-regexp '^credential' ; \
             ls \"$HOME\"/.git-credentials \"$HOME\"/.config/gh/hosts.yml 2>/dev/null; true",
        );
        assert_eq!(creds, "0", "no forge credential on the runner");
        (kept, on_the_runner)
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    let (kept, on_the_runner) = kept;
    assert!(
        kept.iter()
            .any(|one| one.name == "refs/remotes/builds/fix-login" && one.oid == on_the_runner),
        "{kept:?}"
    );
    assert_eq!(
        git(&desk, &["rev-parse", "fix-login"]),
        fix_here,
        "the branch here is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(desk.join("login.rs")).unwrap(),
        "fn log_in() {}\n",
        "the work tree here is untouched"
    );
    drop(removes);
}
