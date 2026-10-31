// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis profile remove` reaches the pools' runners** (#4141), as Remove in Settings › You ›
//! Profiles does: each Docker pool's runner is asked over its link to forget the profile, its
//! sign-in there with it, and the profile comes off each pool that named it. A runner is never
//! woken for it, and one that is away is asked again at the next tidy.
//!
//! The link key is read only with the person's own pools at hand, on an engine shown to be
//! theirs, and never in a chat: a chat removing a profile leaves the runners to the next tidy.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use purlis_core::dockerpool::Docker;
use purlis_core::runners::{self, Kept};
use purlis_host::protocol::hostlink::{self, Handshake, Identity, Liveness};
use purlis_host::protocol::session::{self, Client};
use purlis_host::{CommandConnector, NotChecked};

/// How long a runner has to link and answer: what the window gives one ([`ANSWER_WITHIN`] there).
const ANSWER_WITHIN: Duration = Duration::from_secs(20);

/// **What one pool's runner did of a removed profile**: forgot it; keeps it, running a purlis
/// before 1.52; or was not reached, and why in words.
pub(crate) type Forgot = Result<Option<()>, String>;

/// **Has each pool's runner forget `name`, just removed on this machine**, and takes it off each
/// pool that named it: the lines the person is told, each ending in a newline.
pub(crate) fn forget_off_the_pools(config: &Path, name: &str) -> String {
    let Ok(pools) = runners::pools(config) else {
        return String::new();
    };
    if pools.is_empty() {
        return String::new();
    }
    if crate::memory::in_a_chat() {
        return "the pools' runners forget it at their next tidy: a chat has no link to them\n"
            .to_owned();
    }
    let reach = match reaching(config) {
        Ok(reach) => reach,
        Err(why) => {
            return format!(
                "the pools' runners were not asked, and forget it at their next tidy: {why}\n"
            );
        }
    };
    said_of(config, name, &|pool| {
        let kept = runners::runner_for(config, pool).map_err(|why| why.to_string())?;
        reach.forget(&kept, name)
    })
}

/// **[`forget_off_the_pools`], with what each pool's runner did as `ask` says**: the pools named
/// by a line each. Nothing is said of a runner that forgot it.
fn said_of(config: &Path, name: &str, ask: &dyn Fn(&str) -> Forgot) -> String {
    let Ok(pools) = runners::pools(config) else {
        return String::new();
    };
    let shown = purlis_core::shown::short(name);
    let mut older = Vec::new();
    let mut said = String::new();
    for pool in &pools {
        let pool_name = pool.name();
        // Its sign-in is carried off no draining runner (#4145).
        if let Err(why) = runners::hold_from_carrying(config, pool_name, name) {
            said.push_str(&format!(
                "a drain of pool {pool_name} may carry {shown}: {why}\n"
            ));
        }
        let named = pool
            .profiles()
            .map(|named| named.iter().any(|one| one == name));
        match ask(pool_name) {
            Ok(None) => {
                // Kept where the pool names it, or names none and so kept every profile.
                if named.is_none_or(|named| named) {
                    older.push(pool_name.to_owned());
                }
                continue;
            }
            Ok(Some(())) => {}
            Err(why) => {
                said.push_str(&format!(
                    "pool {pool_name}'s runner is asked again at its next tidy: {why}\n"
                ));
                continue;
            }
        }
        if named == Some(true)
            && let Err(why) = runners::remove_pool_profile(config, pool_name, name, None)
        {
            said.push_str(&format!("pool {pool_name} still names {shown}: {why}\n"));
        }
    }
    if let Some(note) = purlis_core::pools::older_note(&older, &shown) {
        said.push_str(&note);
        said.push('\n');
    }
    said
}

/// What a link to this machine's runners takes.
struct Reach {
    docker: Docker,
    desktop: Identity,
}

/// This machine's `docker`, on an engine shown to be the person's own, and its link key.
fn reaching(config: &Path) -> Result<Reach, String> {
    let engine = Docker::engine_on_this_machine();
    purlis_core::poollogins::owned(engine)?;
    let path = purlis_core::envvar::var("PATH");
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let docker = Docker::on_this_machine(path.as_deref(), home.as_deref()).map_err(|why| why.0)?;
    let keyring = purlis_core::linkkey::this_builds_keyring(config);
    let desktop = purlis_core::linkkey::kept(config, &*keyring)?.into_identity();
    Ok(Reach { docker, desktop })
}

impl Reach {
    /// Has the runner `kept` forget `name` (`forget_profile`, 1.52): the container checked as it
    /// was made before the link, and a stopped one left stopped. `None`, with nothing sent, from
    /// a runner from 1.43 to before 1.52; one before 1.43 keeps no desktop's profile.
    fn forget(&self, kept: &Kept, name: &str) -> Forgot {
        let checking = self.docker.clone();
        let preparing = self.docker.clone();
        let made = kept.made.clone();
        let connector = CommandConnector::checked(
            self.docker.program().to_path_buf(),
            kept.made.connect_args(),
            Arc::new(move || {
                checking
                    .check(&made)
                    .map_err(|why| NotChecked::Refused(why.0))
            }),
        )
        .preparing(Arc::new(move || {
            let (env, cleared, held) = preparing.prepared().map_err(|why| why.0)?.into_parts();
            Ok(purlis_host::Prepared { env, cleared, held })
        }));
        let identity = Identity {
            device: self.desktop.device.clone(),
            key: self.desktop.key.clone(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|why| format!("the link could not be started: {why}"))?;
        let pinned = kept.pinned.clone();
        runtime.block_on(async move {
            let mut links = hostlink::keep_linked(
                connector,
                Handshake::THIS_BUILD,
                identity,
                pinned,
                Liveness::default(),
            );
            tokio::time::timeout(ANSWER_WITHIN, async {
                match links.next().await {
                    Some(hostlink::Linked::Up(link)) => forget_on(Client::new(*link).0, name).await,
                    Some(hostlink::Linked::Down(down)) => Err(down.why),
                    None => Err("the link to the runner ended".to_owned()),
                }
            })
            .await
            .map_err(|_| "the runner did not answer in time".to_owned())?
        })
    }
}

/// `forget_profile` on `client`, where its version has it.
async fn forget_on(client: Client, name: &str) -> Forgot {
    let version = client.version();
    if version < session::KNOWS_KEEP_PROFILE {
        return Ok(Some(()));
    }
    if version < session::KNOWS_FORGET_PROFILE {
        return Ok(None);
    }
    client
        .forget_profile(name)
        .await
        .map(Some)
        .map_err(|why| match why.refusal() {
            Some(refused) => format!(
                "the runner says: {}",
                purlis_core::shown::short(&refused.why)
            ),
            None => purlis_core::shown::short(&why.to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine that keeps Docker pools `builds` and `tests`, written as `runners` reads them.
    fn two_pools(config: &Path, profiles: &str) {
        let dir = purlis_core::machine::dir(config);
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let pool = |name: &str| {
            format!(
                r#"{{"name":"{name}","kind":"docker","image":"ghcr.io/purlis/purlis-runner@sha256:{}"{profiles}}}"#,
                "0".repeat(64)
            )
        };
        std::fs::write(
            dir.join(purlis_core::machine::RUNNERS_FILE),
            format!(
                r#"{{"version":1,"runners":[],"pools":[{},{}]}}"#,
                pool("builds"),
                pool("tests")
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.join(purlis_core::machine::RUNNERS_FILE),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
    }

    #[test]
    fn a_runner_that_forgot_is_not_named_and_the_pool_stops_naming_the_profile() {
        let config = tempfile::tempdir().unwrap();
        two_pools(config.path(), r#","profiles":["work","other"]"#);

        let said = said_of(config.path(), "work", &|_| Ok(Some(())));

        assert_eq!(said, "");
        for pool in runners::pools(config.path()).unwrap() {
            assert_eq!(pool.profiles().unwrap(), ["other"]);
        }
    }

    #[test]
    fn it_names_the_pools_whose_runner_runs_a_purlis_before_1_52() {
        let config = tempfile::tempdir().unwrap();
        two_pools(config.path(), r#","profiles":["work"]"#);

        let said = said_of(config.path(), "work", &|_| Ok(None));

        assert!(
            said.contains("Pools builds and tests keep the profile 'work'"),
            "{said}"
        );
        assert!(said.contains("before 1.52"), "{said}");
    }

    #[test]
    fn a_runner_away_is_asked_again_and_its_pool_keeps_the_name_until_then() {
        let config = tempfile::tempdir().unwrap();
        two_pools(config.path(), r#","profiles":["work"]"#);

        let said = said_of(config.path(), "work", &|pool| {
            if pool == "builds" {
                Err("it is stopped".to_owned())
            } else {
                Ok(Some(()))
            }
        });

        assert_eq!(
            said,
            "pool builds's runner is asked again at its next tidy: it is stopped\n"
        );
        let pools = runners::pools(config.path()).unwrap();
        assert_eq!(pools[0].profiles().unwrap(), ["work"]);
        assert_eq!(pools[1].profiles().unwrap(), Vec::<String>::new());
    }
}
