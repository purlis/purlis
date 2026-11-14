// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A login's values reach a pool on another machine only once the person said only they sign
//! in there, bound to the machine as pinned** (#6271, ADR 0123 §5): the remote parity job's pool,
//! its runner made on the Docker behind the job's sshd and linked through the pool's own forward,
//! judged by the one check every browser login and tool value goes by
//! (`poollogins::Bound::over_the_link`). Unmarked, its runner is sent nothing and the chat is told
//! how the person marks it. Marked, a login opted in is bound to the runner the link reaches, and
//! to the pin. The pin trusted again, the mark is gone and the login asks again. Run by the remote
//! parity job; elsewhere it runs nothing.
//!
//! It checks the gate, not a fill: a fill needs an http(s) page with a field at an origin the
//! runner's browser may reach, and the job's other machine serves none (its registry holds only
//! the runner's image). What a fill does past the gate is the same for any runner over a link
//! (`crates/host/tests/it/a_runner_runs_its_windows_browsers_as_its_desktop_relays_them.rs`).

use purlis_core::poollogins::{Bound, OptIns, Standing};

#[test]
fn in_a_remote_docker_pool() {
    let Some((_target, _machine, pool, again)) = super::a_remote_docker_pool() else {
        return;
    };
    let config = again.config.as_path();
    let reached = format!("{pool}/{pool}");
    let judged = || Bound::over_the_link(config, &pool, Some(&reached));
    let kept = purlis_core::runners::pools(config)
        .expect("the pools read")
        .into_iter()
        .find(|one| one.name() == pool)
        .expect("the pool is kept");
    let remote = kept.remote_engine().expect("on another machine").clone();
    let host = remote.host().to_owned();

    // Not marked: nothing is sent, and the chat is told how the person marks it.
    let unmarked = purlis_core::poollogins::on_another_machine_said(&pool, &host);
    let refused = judged().expect_err("an unmarked pool's runner is sent nothing");
    assert_eq!(refused, unmarked);
    assert!(
        refused.contains(&format!("Who can sign in to {host}?")),
        "{refused}"
    );

    // Marked: bound to the runner the link reaches, with the key it was paired with, and to the pin.
    purlis_core::runners::set_remote_owned(config, &pool, true, &remote.pin_json())
        .expect("the person marks it");
    let bound = judged().expect("a marked pool's runner may be sent a login");
    let runner = purlis_core::runners::runner_for(config, &pool).expect("its runner is kept");
    assert_eq!(bound.runner, runner.pinned.device);
    assert_eq!(bound.engine, remote.pinned_said());
    let logins = OptIns::default();
    logins.allow("steward", "forge", bound.clone());
    assert_eq!(
        logins.standing("steward", "forge", &bound),
        Standing::Allowed
    );

    // The machine trusted again with another pin: the mark is gone, and the login asks again
    // once the person marks it anew.
    let purlis_core::runners::RemoteEngine::Ssh(trusted) = remote.clone() else {
        panic!("the job's pool is reached over SSH");
    };
    let mut repinned = trusted.clone();
    repinned.pin.engine = purlis_core::sshroute::EnginePin::Docker("ANOTHER:ENGINE".to_owned());
    purlis_core::runners::pin_again(config, &pool, &repinned.clone().into()).expect("pinned again");
    // Exactly the unmarked pool's refusal: the mark went with the pin, not some other refusal.
    let refused = judged().expect_err("a pin change clears the mark");
    assert_eq!(refused, unmarked);
    // An answer given for the pin the person was shown before is refused: they answer anew.
    purlis_core::runners::set_remote_owned(config, &pool, true, &remote.pin_json())
        .expect_err("the old pin is no longer the one on disk");
    purlis_core::runners::set_remote_owned(config, &pool, true, &repinned.pin.to_json())
        .expect("marked anew");
    let now = judged().expect("marked for the new pin");
    assert_eq!(logins.standing("steward", "forge", &now), Standing::Changed);

    // The pin the machine really holds, so the pool's runner is removed there as the test ends.
    purlis_core::runners::pin_again(config, &pool, &trusted.into()).expect("trusted as it is");
}
