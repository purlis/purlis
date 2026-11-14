// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool`** (#4284, spec #4275): a pool's limits, for a script or an advanced person,
//! with the names and the checks the window and the file have, all read from
//! [`purlis_core::poollimits`].
//!
//! **In a chat it changes nothing** (Q6, the person's ruling of 2026-11-01 on PC-1): the value is
//! handed to the app as a proposal, which only the person applies. A chat that can't reach the
//! app is refused, and nothing here falls back to writing the file.

use clap::Subcommand;
use purlis_core::poollimits::{self, Limit};
use purlis_core::poolsetup::{
    self,
    asked::{Asked, Created},
};
use purlis_core::runners;

#[derive(Subcommand)]
pub enum PoolCommand {
    /// List a pool's limits: each one's key, its value, and whether the value is yours.
    Limits {
        /// The pool's name.
        pool: String,
        /// One JSON object for a script.
        #[arg(long)]
        json: bool,
    },
    /// Add a Docker pool: its runner is made from the image and paired here.
    ///
    /// This is what Pool setup's Create button runs, and Pool setup picks it up. A team's own
    /// image is checked against the runner image contract first. With `--on ssh://…`, the pool's
    /// Docker is on that machine, reached over SSH with your own SSH settings and agent: purlis
    /// shows the key the machine presents, and makes the pool only once you give its fingerprint
    /// with `--trust`. With `--on tcp://…`, it is reached over Docker's TLS API with the CA,
    /// client certificate and key you name: purlis shows the CA's fingerprint, and makes the pool
    /// only once you give it with `--trust`. Plain tcp:// without them is refused.
    Add {
        /// The pool's name: lower-case letters, digits and dashes.
        name: String,
        /// The runner image, by its digest. See what this does in Pool setup names purlis's own.
        #[arg(long)]
        image: String,
        /// How many CPUs its runner may use, to the thousandth.
        #[arg(long)]
        cpus: Option<String>,
        /// The memory its runner may use, in MiB.
        #[arg(long)]
        memory_mib: Option<u32>,
        /// How many processes its runner may run at once.
        #[arg(long)]
        processes: Option<u32>,
        /// The size of its runner's /tmp, in MiB.
        #[arg(long)]
        tmp_mib: Option<u32>,
        /// How long its runner waits without a link before it suspends: 12h, 90m.
        #[arg(long)]
        suspend_after: Option<String>,
        /// The most frames a second its runner's browsers are watched at.
        #[arg(long)]
        frames: Option<u32>,
        /// The widest frame its runner's browsers are watched at, in pixels.
        #[arg(long)]
        width: Option<u32>,
        /// The machine its Docker is on: ssh://[user@]host[:port][/socket], or
        /// tcp://host[:port] with --tls-ca, --tls-cert and a key. Plain tcp:// is refused.
        #[arg(long, value_name = "ROUTE")]
        on: Option<String>,
        /// The fingerprint of that machine's host key, as you checked it there
        /// (SHA256:…, from `ssh-keygen -lf`), or over TLS of its CA (sha256:…, or as
        /// `openssl x509 -noout -fingerprint -sha256` prints it).
        #[arg(long, value_name = "FINGERPRINT", requires = "on")]
        trust: Option<String>,
        /// The key file ssh signs in to that machine with, as a Podman connection names it
        /// (`podman system connection list`). Without it, your SSH agent and settings.
        #[arg(long, value_name = "FILE", requires = "on")]
        identity: Option<String>,
        /// The files Docker's TLS API is reached with (#6267).
        #[command(flatten)]
        tls: Box<TlsArgs>,
    },
    /// Record profiles of this machine's on a pool, for its chats to use. Its runner keeps them
    /// the next time their sign-ins are asked, and `purlis profile sign-in <profile> --pool
    /// <pool>` signs one in there.
    ///
    /// This is what Pool setup's Sign-ins button records, with the same checks of the profiles,
    /// and Pool setup picks it up. It sends nothing. A pool that names no profiles keeps your project's default beside them, so run
    /// it in your project's folder.
    AddProfile {
        /// The pool's name.
        pool: String,
        /// The profiles, by name, as `purlis profile list` shows them.
        #[arg(required = true)]
        profiles: Vec<String>,
    },
    /// Take a profile off a pool: its runner forgets it first, and its sign-in there with it, and
    /// then the pool leaves it out.
    ///
    /// This is what Pool setup's Sign-ins button does to a profile you untick. Only the running
    /// purlis window can reach the runner, so you confirm it there, against the code this prints,
    /// and with no window it refuses. A pool that names no profiles yet keeps your project's
    /// default: take that off in Pool setup.
    RemoveProfile {
        /// The pool's name.
        pool: String,
        /// The profile, by name, as `purlis pool setup` shows it.
        profile: String,
    },
    /// Pool setup: each step's state and what to do next, for a pool or a new one. It only reads.
    Setup {
        /// The pool. Without it, a new Docker pool.
        name: Option<String>,
        /// One JSON object for a script.
        #[arg(long)]
        json: bool,
    },
    /// Check a pool on another machine: that its machine and engine are the ones you trusted.
    /// It exits 1 when they aren't, or when the machine can't be reached, saying why. It opens
    /// the pool's connection, and changes nothing.
    Doctor {
        /// The pool's name.
        name: String,
    },
    /// List the runners left on another Docker engine: a removal that found Docker answering as
    /// another engine than the one that made a runner forgot it here, and left its container
    /// and home volume there.
    ///
    /// With `--remove`, removes each one whose engine Docker answers as now, and forgets it.
    LeftBehind {
        /// Remove each one whose engine Docker answers as now.
        #[arg(long)]
        remove: bool,
    },
    /// Set one of a pool's limits, or return it to the default with `--reset`.
    ///
    /// In a chat the app started, this writes nothing: the person is asked in the window, and
    /// only they apply it.
    Limit {
        /// The pool's name.
        pool: String,
        /// The limit's key, as `purlis pool limits` lists it.
        key: String,
        /// The new value, a whole number in the limit's unit.
        #[arg(allow_hyphen_values = true, required_unless_present = "reset")]
        value: Option<String>,
        /// Return it to following the default.
        #[arg(long, conflicts_with = "value")]
        reset: bool,
    },
}

/// Runs `command`, and says why not where it can't.
pub fn run(command: &PoolCommand) -> std::process::ExitCode {
    match answered(command) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(why) => {
            crate::voice::err(why);
            std::process::ExitCode::FAILURE
        }
    }
}

fn answered(command: &PoolCommand) -> Result<u8, String> {
    let config = || {
        purlis_core::machine::config_root()
            .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
    };
    let in_a_chat = crate::gradelink::in_a_chat(&purlis_core::envvar::var);
    let in_a_chat = if writes_a_pool(command) {
        in_a_chat || below_a_chat()?
    } else {
        in_a_chat
    };
    match command {
        // A chat doesn't reach this machine's Docker or a runner, so it asks nothing of them.
        PoolCommand::Setup { .. } if in_a_chat => Err(
            "Pool setup reads this machine's Docker and pools, which a chat doesn't reach. Run \
             purlis pool setup in your own terminal."
                .to_owned(),
        ),
        PoolCommand::Doctor { .. } if in_a_chat => Err(
            "purlis pool doctor reaches the pool's machine, which a chat doesn't reach. Run it \
             in your own terminal."
                .to_owned(),
        ),
        // A chat never chooses which of the person's accounts a pool's runner holds.
        PoolCommand::AddProfile { .. } if in_a_chat => Err(
            "Recording a profile on a pool is the person's choice, which a chat doesn't make. \
             Run it in your own terminal, or use Pool setup in the window."
                .to_owned(),
        ),
        PoolCommand::RemoveProfile { .. } if in_a_chat => Err(
            "Taking a profile off a pool deletes its sign-in there, which is the person's choice \
             and not a chat's. Run it in your own terminal, or use Pool setup in the window."
                .to_owned(),
        ),
        PoolCommand::LeftBehind { .. } if in_a_chat => Err(
            "The runners left on another engine are read from this machine's pools, and removed \
             with its Docker, which a chat doesn't reach. Run purlis pool left-behind in your own \
             terminal."
                .to_owned(),
        ),
        PoolCommand::Add { .. } if in_a_chat => Err(
            "Adding a pool uses this machine's Docker, which a chat doesn't reach. Run it in your \
             own terminal, or use Pool setup in the window."
                .to_owned(),
        ),
        PoolCommand::Add {
            name,
            image,
            cpus,
            memory_mib,
            processes,
            tmp_mib,
            suspend_after,
            frames,
            width,
            on,
            trust,
            identity,
            tls,
        } => {
            let tls = tls_files(&tls.tls_ca, &tls.tls_cert, &tls.tls_key, &tls.tls_key_in)?;
            let asked = Asked {
                name: name.clone(),
                image: image.clone(),
                cpus: cpus.clone(),
                memory_mib: *memory_mib,
                processes: *processes,
                tmp_mib: *tmp_mib,
                suspend_after: suspend_after.clone(),
                frames: *frames,
                width: *width,
                on: on.clone(),
                trust: trust.clone(),
                identity: identity.clone(),
                tls,
            };
            added(&config()?, &asked)
        }
        PoolCommand::AddProfile { pool, profiles } => {
            let project = project_here();
            let config = config()?;
            poolsetup::signins::record(&config, project.as_deref(), pool, profiles)?;
            // Every profile it keeps now, a project's default added beside them included.
            let keeps = runners::profiles_of(&config, pool)
                .map_err(|why| why.to_string())?
                .unwrap_or_default();
            crate::voice::info(format!(
                "Pool {pool} keeps {} now. Its runner keeps them the next time their sign-ins are \
                 asked, and Pool setup's Sign-ins step picks this up.",
                keeps.join(", ")
            ));
            Ok(0)
        }
        PoolCommand::RemoveProfile { pool, profile } => {
            let config = config()?;
            poolsetup::signins::may_take_off(&config, pool, profile)?;
            let note = taken_off(&config, pool, profile)?;
            crate::voice::ok(format!(
                "Pool {pool} no longer keeps '{profile}', and Pool setup's Sign-ins step picks \
                 this up."
            ));
            if let Some(note) = note {
                crate::voice::info(note);
            }
            Ok(0)
        }
        PoolCommand::LeftBehind { remove } => {
            left_behind(&config()?, *remove)?;
            Ok(0)
        }
        PoolCommand::Setup { name, json } => {
            crate::poolsetup::answered(name.as_deref(), *json)?;
            Ok(0)
        }
        PoolCommand::Doctor { name } => crate::poolsetup::doctor(name),
        // A chat can't read the pool's file, so it is told what can be asked for, and no value.
        PoolCommand::Limits { json, .. } if in_a_chat => {
            let held = runners::PoolLimitsHeld {
                chosen: poollimits::Chosen::default(),
                refused: Vec::new(),
                docker: false,
            };
            if *json {
                let mut said = limits_json(&held);
                said["defaultsOnly"] = true.into();
                if let Some(rows) = said["limits"].as_array_mut() {
                    for row in rows {
                        row["inForce"] = serde_json::Value::Null;
                    }
                }
                println!("{said}");
            } else {
                for line in limits_lines(&held) {
                    println!("{line}");
                }
                crate::voice::info(
                    "In a chat these are purlis's defaults, not this pool's values: only the \
                     person sees those, in the window.",
                );
            }
            Ok(0)
        }
        PoolCommand::Limits { pool, json } => {
            let held = runners::limits_held_of(&config()?, pool).map_err(|why| why.to_string())?;
            if *json {
                println!("{}", limits_json(&held));
            } else {
                for line in limits_lines(&held) {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        PoolCommand::Limit {
            pool,
            key,
            value,
            reset: _,
        } => {
            // A limit that can't change says so, why, and what to do instead (#4281).
            if let Some(fixed) = poollimits::fixed().iter().find(|one| one.key == *key) {
                return Err(format!("{}: {}", fixed.plain, fixed.said()));
            }
            let limit = poollimits::of_key(key).ok_or_else(|| {
                format!(
                    "a pool has no limit named {}: `purlis pool limits <pool>` lists them",
                    purlis_core::shown::short(key)
                )
            })?;
            let value = value
                .as_deref()
                .map(|typed| limit.check_typed(typed, None))
                .transpose()?;
            if in_a_chat {
                return crate::brokered::propose_limit(pool, limit, value);
            }
            runners::set_pool_limit(&config()?, pool, limit.id, value)
                .map_err(|why| why.to_string())?;
            let chosen = runners::chosen_limits_of(&config()?, pool).unwrap_or_default();
            crate::voice::ok(saved_said(pool, limit, value, &chosen));
            Ok(0)
        }
    }
}

/// The project whose folder this runs in, if it runs in one.
pub(crate) fn project_here() -> Option<std::path::PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| purlis_core::plane::find_root(&cwd).ok())
}

/// Whether `command` writes this machine's pools.
fn writes_a_pool(command: &PoolCommand) -> bool {
    match command {
        PoolCommand::Add { .. }
        | PoolCommand::AddProfile { .. }
        | PoolCommand::RemoveProfile { .. }
        | PoolCommand::Limit { .. } => true,
        PoolCommand::LeftBehind { remove } => *remove,
        PoolCommand::Limits { .. } | PoolCommand::Setup { .. } | PoolCommand::Doctor { .. } => {
            false
        }
    }
}

/// **Says each runner left on another engine** (#2587), and with `remove`, removes each one
/// whose engine this machine's Docker answers as now.
fn left_behind(config: &std::path::Path, remove: bool) -> Result<(), String> {
    use runners::left;
    if !remove {
        let kept = left::read(config).map_err(|why| why.to_string())?;
        if kept.is_empty() {
            crate::voice::info("No runner is left on another Docker engine.");
            return Ok(());
        }
        for one in &kept {
            crate::voice::info(format!(
                "Runner {} of pool {} is on Docker engine {}: {}",
                one.name(),
                one.pool,
                one.engine,
                one.how()
            ));
        }
        crate::voice::info(
            "Run purlis pool left-behind --remove once Docker answers as that engine again, or \
             run those commands where it does.",
        );
        return Ok(());
    }
    let docker = purlis_core::poolsetup::docker_here()
        .ok_or_else(|| "Docker isn't on this machine, so nothing was removed.".to_owned())?;
    let outcomes = left::remove(config, &docker).map_err(|why| why.to_string())?;
    if outcomes.is_empty() {
        crate::voice::info("No runner is left on another Docker engine.");
    }
    let mut all = true;
    for (one, done) in outcomes {
        match done {
            Ok(()) => crate::voice::ok(format!(
                "Runner {} of pool {} is removed from Docker engine {}.",
                one.name(),
                one.pool,
                one.engine
            )),
            Err(why) => {
                all = false;
                crate::voice::err(why);
            }
        }
    }
    if all {
        Ok(())
    } else {
        Err("Some runners are still left on another engine; purlis keeps them.".to_owned())
    }
}

/// Whether this runs in a chat, by the same check as secret setup. Where that can't tell, nothing
/// is written.
fn below_a_chat() -> Result<bool, String> {
    use purlis_core::secrets::{Ctx, Env, setup};
    let root = project_here()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "this command's folder could not be read, so nothing changed.".to_owned())?;
    match setup::in_a_chat(&Ctx::new(&root, Env::from_process())) {
        setup::Where::Outside => Ok(false),
        setup::Where::Inside => Ok(true),
        setup::Where::Unsure(why) => Err(format!(
            "purlis could not tell whether this runs in a chat, so nothing changed: {}. Use Pool \
             setup in the window.",
            // Whole: it ends with what clears the doubt.
            purlis_core::shown::one_line(&why, 1000)
        )),
    }
}

/// **Has the running window take `name` off pool `pool`**, on the person's yes there, which alone
/// can reach its runner.
#[cfg(unix)]
fn taken_off(config: &std::path::Path, pool: &str, name: &str) -> Result<Option<String>, String> {
    use purlis_core::windowline;
    let code = windowline::a_new_code();
    let project = project_here();
    windowline::taken_off(config, project.as_deref(), pool, name, &code, |asked| {
        crate::voice::info(format!(
            "Confirm taking '{name}' off pool {pool} in the purlis window, if it shows the code \
             {code}…"
        ));
        windowline::ask(asked, &mut |line| crate::voice::info(line.to_owned()))
    })
}

/// A runner is never Windows, and no window line reaches one from there.
#[cfg(not(unix))]
fn taken_off(_config: &std::path::Path, pool: &str, name: &str) -> Result<Option<String>, String> {
    Err(format!(
        "taking '{name}' off pool {pool} is confirmed in the purlis window, which this command \
         cannot reach on this system, so nothing changed. Use Pool setup in the window."
    ))
}

/// **Adds the pool `asked` asks for**, as Pool setup's button does.
fn added(config: &std::path::Path, asked: &Asked) -> Result<u8, String> {
    let docker = poolsetup::docker_here()
        .ok_or_else(|| "purlis found no docker on this machine.".to_owned())?;
    // A pool on another machine needs only Docker's command here: its engine is there.
    if asked.on.is_none() {
        docker.answers().map_err(|why| why.0)?;
    }
    let keyring = purlis_core::linkkey::this_builds_keyring(config);
    let desktop = purlis_core::linkkey::desktop(config, &*keyring)?.pinned();
    match poolsetup::asked::create(config, &docker, asked, &desktop)
        .map_err(|why| why.to_string())?
    {
        Created::Added => {
            crate::voice::ok(format!(
                "Pool {} is added, and its runner is made.",
                asked.name
            ));
            Ok(0)
        }
        Created::Breaks(problems) => Err(format!(
            "This image can't be a runner, and nothing was made:\n{}",
            problems.join("\n")
        )),
    }
}

/// What a save says.
fn saved_said(
    pool: &str,
    limit: &Limit,
    value: Option<u64>,
    chosen: &poollimits::Chosen,
) -> String {
    let now = value.map_or_else(
        || {
            format!(
                "back to the default, {}",
                limit.unit.said(chosen.default_of(limit.id))
            )
        },
        |value| limit.unit.said(value),
    );
    format!(
        "Pool {pool}: {} is {now}. {}",
        limit.plain,
        limit.takes_effect.said()
    )
}

/// The limits this kind of pool has, under `held`.
fn listed(held: &runners::PoolLimitsHeld) -> impl Iterator<Item = &'static Limit> + use<> {
    let docker = held.docker;
    let refused: Vec<_> = held.refused.iter().map(|one| one.limit).collect();
    // A value a Docker pool should never have held is still listed, so it can be reset.
    poollimits::all()
        .iter()
        .filter(move |limit| !(docker && limit.clusters_only) || refused.contains(&limit.id))
}

fn limits_lines(held: &runners::PoolLimitsHeld) -> Vec<String> {
    let mut lines = Vec::new();
    for limit in listed(held) {
        let value = held.chosen.in_force(limit.id);
        let whose = if held.chosen.get(limit.id).is_some() {
            "yours"
        } else {
            "default"
        };
        let refused = held
            .refused
            .iter()
            .find(|one| one.limit == limit.id)
            .map(poollimits::RefusedValue::said)
            .or_else(|| held.chosen.over_highest_said(limit.id))
            .unwrap_or_default();
        lines.push(format!(
            "{}\t{}\t{whose}\t{}\t{refused}",
            limit.key,
            limit.unit.said(value),
            limit.plain
        ));
    }
    lines
}

fn limits_json(held: &runners::PoolLimitsHeld) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = listed(held)
        .map(|limit| {
            serde_json::json!({
                "key": limit.key,
                "plain": limit.plain,
                "unit": limit.unit.word(),
                "default": held.chosen.default_of(limit.id),
                "defaultByTeam": held.chosen.default_by_team(limit.id),
                "highest": held.chosen.highest(limit.id),
                "set": held.chosen.get(limit.id),
                "inForce": held.chosen.in_force(limit.id),
                "applies": limit.takes_effect.said(),
                "refused": held.refused.iter().find(|one| one.limit == limit.id)
                    .map(purlis_core::poollimits::RefusedValue::said)
                    .or_else(|| held.chosen.over_highest_said(limit.id)),
            })
        })
        .collect();
    serde_json::json!({ "limits": rows })
}

/// **`purlis pool add`'s flags for a machine reached over Docker's TLS API** (#6267).
#[derive(Debug, clap::Args)]
pub struct TlsArgs {
    /// Over TLS: the CA Docker's certificates there are signed with.
    #[arg(long, value_name = "FILE", requires = "on")]
    tls_ca: Option<std::path::PathBuf>,
    /// Over TLS: your client certificate.
    #[arg(long, value_name = "FILE", requires = "tls_ca")]
    tls_cert: Option<std::path::PathBuf>,
    /// Over TLS: your client certificate's key, where it is.
    #[arg(
        long,
        value_name = "FILE",
        requires = "tls_cert",
        conflicts_with = "tls_key_in"
    )]
    tls_key: Option<std::path::PathBuf>,
    /// Over TLS: your client certificate's key, kept in a vault entry of this project's
    /// (VAULT/KEY). It is written to disk only for each run of docker, and removed after.
    #[arg(long, value_name = "VAULT/KEY", requires = "tls_cert")]
    tls_key_in: Option<String>,
}

/// **The TLS files `purlis pool add` was given** (#6267), each made absolute against the folder it
/// runs in; a key in a vault is this project's (`VAULT/KEY`). `None` where none was given; refused
/// where some but not all were.
fn tls_files(
    ca: &Option<std::path::PathBuf>,
    cert: &Option<std::path::PathBuf>,
    key: &Option<std::path::PathBuf>,
    key_in: &Option<String>,
) -> Result<Option<purlis_core::tlsroute::Files>, String> {
    if ca.is_none() && cert.is_none() && key.is_none() && key_in.is_none() {
        return Ok(None);
    }
    let here =
        std::env::current_dir().map_err(|why| format!("this folder can't be read: {why}"))?;
    let absolute = |path: &std::path::Path| here.join(path);
    let (Some(ca), Some(cert)) = (ca, cert) else {
        return Err(purlis_core::tlsroute::NO_VERIFY.to_owned());
    };
    let key = match (key, key_in) {
        (Some(key), None) => purlis_core::tlsroute::KeyFrom::File(absolute(key)),
        (None, Some(key_in)) => {
            let (vault, key) = key_in.split_once('/').ok_or_else(|| {
                format!("--tls-key-in is VAULT/KEY, as in docker-box/TLS_KEY, not {key_in}")
            })?;
            let plane = project_here().ok_or_else(|| {
                "run this in the project whose vault holds the key, or name the key's file with \
                 --tls-key"
                    .to_owned()
            })?;
            purlis_core::tlsroute::KeyFrom::Vault {
                plane,
                vault: vault.to_owned(),
                key: key.to_owned(),
            }
        }
        _ => {
            return Err(
                "name your client certificate's key with --tls-key FILE or --tls-key-in VAULT/KEY"
                    .to_owned(),
            );
        }
    };
    purlis_core::tlsroute::Files::new(absolute(ca), absolute(cert), key).map(Some)
}
