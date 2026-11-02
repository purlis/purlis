// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A short task on a runner of its own, for real** (#4569, #4568): the desktop's whole path on a
//! cluster pool, through `Chats` as the app holds it. A short task's start claims a fresh runner,
//! readies its profile there and starts on it; a commit made on that runner comes back into the
//! desktop's clone; closing the task deletes the Claim only after that; and a start that fails
//! leaves no Claim behind.
//!
//! It needs a cluster with purlis's cluster operator installed. `operator/e2e/kind.sh` runs it with
//! `--ignored`, naming in `CLUSTER_POOL_TEST_HARNESS_IMAGE` a runner image whose `claude` is a
//! stand-in that signs in and waits, and in `CLUSTER_POOL_TEST_CONTEXT` the kubeconfig's context.
//! No model answers, and no real cluster is reached. It is ignored anywhere else, and fails when
//! either is missing.

use purlis_core::clusterpool::Kubectl;
use purlis_core::harness_declaration::SignInKind;
use purlis_core::profilestore::{self, Folders, MachineProfile};

use super::*;
use crate::chats::Chats;
use crate::runnerlinks::Machine;

const PROFILE: &str = "work";
const WORKSPACE: &str = "app";
const CLONE: &str = "web";
const BRANCH: &str = "task-work";
const LONG: Duration = Duration::from_secs(300);

fn until<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + LONG;
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "{what}, within {LONG:?}");
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Removes what the test made in the cluster, however it ends.
struct RemovesFromTheCluster {
    kubectl: Kubectl,
    namespace: String,
    pool: String,
}

impl Drop for RemovesFromTheCluster {
    fn drop(&mut self) {
        for args in [
            vec!["delete", "namespace", &self.namespace, "--wait=false"],
            vec![
                "delete",
                "pools.runners.purlis.dev",
                &self.pool,
                "--wait=false",
            ],
        ] {
            let _ = self.kubectl.run(&self.kubectl.args(&args), None, 64 * 1024);
        }
    }
}

/// The Pool of `image` in a namespace of the test's own, added on `machine` as the window adds one
/// and marked as the person's own, so a profile may reach its runners.
fn a_pool_on_the_cluster(
    machine: &Machine,
    kubectl: &Kubectl,
    context: &str,
    image: &str,
) -> (RemovesFromTheCluster, String) {
    let minted = purlis_core::reopen::mint().to_lowercase();
    let suffix = &minted[minted.len() - 10..];
    let (namespace, pool) = (format!("e2e-fresh-{suffix}"), format!("e2e-fresh-{suffix}"));
    let removes = RemovesFromTheCluster {
        kubectl: kubectl.clone(),
        namespace: namespace.clone(),
        pool: pool.clone(),
    };
    kubectl
        .run(
            &kubectl.args(&["create", "namespace", &namespace]),
            None,
            64 * 1024,
        )
        .expect("a namespace of the test's own");
    let definition = serde_json::json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": pool },
        "spec": { "image": image, "keptVolume": { "size": "1Gi" } },
    });
    kubectl
        .run(
            &kubectl.args(&["apply", "--filename", "-"]),
            Some(&definition.to_string()),
            64 * 1024,
        )
        .expect("the pool");
    let operator = until("the pool names the operator's key", || {
        kubectl.pool_pin(&pool).ok()
    });
    let kubeconfig = std::env::var_os("KUBECONFIG").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").expect("a home")).join(".kube/config"),
        PathBuf::from,
    );
    let team = purlis_core::runners::ClusterPool::new(
        "team",
        &kubeconfig,
        context,
        &pool,
        &namespace,
        operator,
    )
    .expect("a cluster pool");
    let desktop = purlis_core::linkkey::desktop(
        &machine.config,
        &*purlis_core::linkkey::this_builds_keyring(&machine.config),
    )
    .expect("this machine's link key")
    .pinned();
    purlis_core::pools::add_cluster(
        &machine.config,
        &purlis_core::pools::kubectl_here(&machine.config).expect("kubectl on this machine"),
        &team,
        &desktop,
        LONG,
    )
    .expect("the pool is added");
    purlis_core::runners::set_cluster_owned(&machine.config, "team", true)
        .expect("it is marked as the person's own");
    (removes, namespace)
}

/// The desktop's profile: signed in with a key, so a short task on it keeps nothing.
fn a_profile_with_a_key(machine: &Machine, root: &Path, folder: &Path) {
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
            pinned: None,
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

/// A short task of `workspace`, on the profile with a key.
fn a_short_task(
    root: &Path,
    graded: bool,
) -> (purlis_core::reopen::Chat, purlis_core::start::Ready) {
    let chat = purlis_core::reopen::Chat {
        program: "claude".to_owned(),
        cwd: Some(root.join("workspaces").join(WORKSPACE)),
        name: "a short task".to_owned(),
        profile: Some(PROFILE.to_owned()),
        persona: Some("steward".to_owned()),
        graded,
        ..Default::default()
    };
    let ready = purlis_core::start::Ready {
        program: "claude".to_owned(),
        harness: Some(purlis_core::harness::Harness::ClaudeCode),
        sign_in: Some(purlis_core::profilesignin::AtStart {
            kind: SignInKind::ApiKey,
            billed_to: "your key".to_owned(),
            credentials: Vec::new(),
        }),
        ..crate::chats::tests::a_shell_ready()
    };
    (chat, ready)
}

/// `script`, run in the fresh runner's pod.
fn on_the_runner(
    kubectl: &Kubectl,
    kept: &purlis_core::runners::ClusterKept,
    namespace: &str,
    script: &str,
) -> String {
    kubectl
        .run(
            &kubectl.args(&[
                "exec",
                "--namespace",
                namespace,
                "--container",
                purlis_core::clusterpool::RUNNER_CONTAINER,
                &format!("pod/{}", kept.pod),
                "--",
                "/bin/sh",
                "-c",
                script,
            ]),
            None,
            64 * 1024,
        )
        .unwrap_or_else(|why| panic!("the script ran on the runner: {why}"))
}

fn the_claims_of(kubectl: &Kubectl, namespace: &str) -> Vec<String> {
    let said = kubectl
        .run(
            &kubectl.args(&[
                "get",
                "claims.runners.purlis.dev",
                "--namespace",
                namespace,
                "--output",
                "name",
            ]),
            None,
            64 * 1024,
        )
        .expect("the claims read");
    said.lines().map(str::to_owned).collect()
}

fn the_commit_of(repo: &Path, reference: &str) -> Option<String> {
    let done =
        purlis_core::forklock::output(std::process::Command::new("git").current_dir(repo).args([
            "rev-parse",
            "--verify",
            "--quiet",
            reference,
        ]))
        .expect("git runs");
    done.status
        .success()
        .then(|| String::from_utf8_lossy(&done.stdout).trim().to_owned())
}

#[test]
#[ignore = "needs a cluster with purlis's cluster operator: operator/e2e/kind.sh"]
fn a_short_task_runs_on_a_runner_of_its_own_and_its_commits_are_home_before_its_claim_goes() {
    let image = std::env::var("CLUSTER_POOL_TEST_HARNESS_IMAGE")
        .expect("CLUSTER_POOL_TEST_HARNESS_IMAGE names the runner image by digest");
    let context = std::env::var("CLUSTER_POOL_TEST_CONTEXT")
        .expect("CLUSTER_POOL_TEST_CONTEXT names the cluster's kubeconfig context");
    let config = tempfile::tempdir().expect("the desktop's config home");
    let config_path = config.path().canonicalize().expect("its folder");
    let machine = Machine {
        config: config_path.clone(),
        docker: purlis_core::dockerpool::Docker::at(std::ffi::OsString::from("docker")),
        keyring: Arc::new(purlis_core::secrets::keyring::FileStore::at(
            config_path.join("keyring.json"),
        )),
    };
    let kubectl = Kubectl::at("kubectl").with_context(&context);
    let (_removes, namespace) = a_pool_on_the_cluster(&machine, &kubectl, &context, &image);

    // A window whose project's workspace runs its chats on that pool, as the app routes them.
    let WindowsHost {
        runtime,
        folder,
        data,
        cuts,
        a_connection,
    } = a_windows_host_for(&machine);
    let mut target = a_window_over(
        a_connection(),
        Vec::new(),
        data.path().to_path_buf(),
        Runs::AsRouted {
            config: config_path.clone(),
        },
    );
    let root = target.root.clone();
    a_repository(&root);
    let clone = root.join("workspaces").join(WORKSPACE).join(CLONE);
    std::fs::create_dir_all(&clone).expect("its clone");
    std::fs::write(clone.join("README.md"), "# web\n").expect("its file");
    a_repository(&clone);
    purlis_core::machine::update(&config_path, |store| {
        store.remember(&root, 1);
        store
            .set_chats_run_on(&root, WORKSPACE, Some("team"))
            .expect("the workspace runs on the pool");
    })
    .expect("the store is written");
    let settings = tempfile::tempdir().expect("the profile's folder");
    a_profile_with_a_key(&machine, &root, settings.path());

    let sessions = std::mem::replace(
        &mut target.sessions,
        Box::new(HostedSessions::new(Arc::new(Connection::default()), &root)),
    );
    let mut chats = Chats::on_host(Box::new(|_| {}), sessions, root.clone());
    chats.keeping_pools_in(Some(config_path.clone()));
    let pool_own_claims = the_claims_of(&kubectl, &namespace);
    assert_eq!(pool_own_claims.len(), 1, "{pool_own_claims:?}");

    // A task that is not a short one stays on the pool's own runner: asked of it, nothing is made.
    // A short one is given a runner of its own, with its profile readied there.
    let (chat, ready) = a_short_task(&root, false);
    let session = crate::hostedsessions::a_short_task(|| chats.start_ready(&chat, &ready, SIZE))
        .unwrap_or_else(|why| panic!("the short task starts: {why}"));
    let fresh = purlis_core::runners::fresh_runners(&config_path).expect("the file reads");
    assert_eq!(fresh.len(), 1, "it has a runner of its own: {fresh:?}");
    let kept = fresh.into_iter().next().expect("one");
    assert_eq!(
        the_claims_of(&kubectl, &namespace).len(),
        2,
        "its Claim is beside the pool's own"
    );
    assert!(chats.on_its_own_runner(session));
    let there = on_the_runner(
        &kubectl,
        &kept,
        &kept_namespace(&kubectl, &config_path, &kept),
        &format!("find \"$HOME\" -type d -path '*/workspaces/{WORKSPACE}/{CLONE}' | head -n 1"),
    );
    let there = there.trim().to_owned();
    assert!(!there.is_empty(), "its clone reached its runner");

    // The task commits on its runner, on a branch of its own.
    let work = on_the_runner(
        &kubectl,
        &kept,
        &kept_namespace(&kubectl, &config_path, &kept),
        &format!(
            "cd '{there}' && git checkout -q -b {BRANCH} && echo done > task.txt && git add . \
             && git -c user.name=task -c user.email=task@example.com commit -q -m 'the task' \
             && git rev-parse HEAD"
        ),
    );
    let work = work.trim().to_owned();

    // Closed, its work comes home and only then does its Claim go.
    chats.close(session).expect("the task closes");
    let runner = kept.link_name();
    until("its Claim goes", || {
        (the_claims_of(&kubectl, &namespace).len() == 1).then_some(())
    });
    assert_eq!(
        the_commit_of(&clone, &format!("refs/remotes/{runner}/{BRANCH}")),
        Some(work),
        "its commit is home"
    );
    assert_eq!(
        the_commit_of(&clone, &format!("refs/remotes/{runner}/main")),
        None,
        "what this machine has anyway is not kept twice"
    );
    until("its record goes", || {
        purlis_core::runners::fresh_runners(&config_path)
            .is_ok_and(|kept| kept.is_empty())
            .then_some(())
    });

    // A start that fails after the runner was made leaves no Claim: a grade's chat is refused.
    let (graded, ready) = a_short_task(&root, true);
    let refused = crate::hostedsessions::a_short_task(|| chats.start_ready(&graded, &ready, SIZE))
        .expect_err("a grade's chat starts on no runner");
    assert!(refused.contains("grade"), "{refused}");
    until("the failed start's Claim goes", || {
        (the_claims_of(&kubectl, &namespace) == pool_own_claims).then_some(())
    });

    // **A window started again** (#4569): a task left running is found on its own runner, and one
    // whose chat this window does not hold has what it did brought home, and goes.
    let (task, ready) = a_short_task(&root, false);
    let session = crate::hostedsessions::a_short_task(|| chats.start_ready(&task, &ready, SIZE))
        .unwrap_or_else(|why| panic!("the second short task starts: {why}"));
    let kept = purlis_core::runners::fresh_runners(&config_path)
        .expect("the file reads")
        .into_iter()
        .next()
        .expect("it has a runner of its own");
    let namespace_there = kept_namespace(&kubectl, &config_path, &kept);
    let there = on_the_runner(
        &kubectl,
        &kept,
        &namespace_there,
        &format!("find \"$HOME\" -type d -path '*/workspaces/{WORKSPACE}/{CLONE}' | head -n 1"),
    );
    let work = on_the_runner(
        &kubectl,
        &kept,
        &namespace_there,
        &format!(
            "cd '{}' && git checkout -q -b {BRANCH}-2 && echo done > two.txt && git add . \
             && git -c user.name=task -c user.email=task@example.com commit -q -m 'the task' \
             && git rev-parse HEAD",
            there.trim()
        ),
    );
    for cut in cuts.lock().expect("the cuts").drain(..) {
        cut.abort();
    }
    let connection = a_connection();
    let again = crate::routedsessions::RoutedSessions::of_the_project(
        Box::new(HostedSessions::new(Arc::clone(&connection), &root)),
        &connection,
        &config_path,
        &root,
    );
    let mut chats_again = Chats::on_host(Box::new(|_| {}), Box::new(again), root.clone());
    chats_again.keeping_pools_in(Some(config_path.clone()));
    let left = purlis_core::reopen::Chat {
        number: Some(session),
        identity: purlis_core::reopen::Identity {
            id: kept.chat.clone(),
            ..Default::default()
        },
        ..task
    };
    assert!(
        chats_again.left_running(&left),
        "the task is found on its own runner"
    );
    chats_again.close_what_was_not_reattached();
    until("the swept runner's Claim goes", || {
        (the_claims_of(&kubectl, &namespace) == pool_own_claims).then_some(())
    });
    assert_eq!(
        the_commit_of(
            &clone,
            &format!("refs/remotes/{}/{BRANCH}-2", kept.link_name())
        ),
        Some(work.trim().to_owned()),
        "its commit came home before its Claim went"
    );

    drop(chats_again);
    drop(chats);
    drop(target);
    runtime.shutdown_timeout(Duration::from_secs(5));
    drop((folder, data));
}

/// The namespace of `kept`'s pod, as the operator vouches for it.
fn kept_namespace(
    kubectl: &Kubectl,
    config: &Path,
    kept: &purlis_core::runners::ClusterKept,
) -> String {
    let (pool, _) = purlis_core::runners::cluster_runner_for(config, &kept.link_name())
        .expect("the file reads")
        .expect("its runner");
    purlis_core::clusterpool::vouched_for(kubectl, &pool.pin().expect("its pin"), &kept.claimed)
        .expect("the statement checks")
        .expect("it is vouched for")
        .runner_namespace
}
