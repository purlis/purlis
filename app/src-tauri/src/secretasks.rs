// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Secret ask, answered in the window** (#5068, #5071, ADR 0109): the window's half of
//! [`purlis_core::secrets::ask`].
//!
//! A chat's brokered `secret exec` whose vault holds no value under a key it hands over waits on
//! a Secret ask (`crate::vaults::run_brokered`), listed with every other ask
//! (`crate::asking::secret_ask`). The person fills it, with a value typed into the ask's masked
//! field, or chooses Not now ([`answer_secret_ask`]); or it picks an entry it keeps already, which
//! the asked key comes to read as a Linked entry ([`link_secret_ask`], #5073), from the entries
//! [`secret_ask_entries`] lists vault by vault.
//!
//! - **Only the window answers.** [`answer_secret_ask`] and [`link_secret_ask`] are Tauri
//!   commands of the window's, never served on the link to `charterd`
//!   (`purlis_session_protocol::ui::WINDOW_ONLY`), and each answers as the window's own scope ([`crate::asking::the_window`]), the one that holds
//!   `AnswerASecret`. The core refuses any other answerer.
//! - **The value goes to the vault's provider and nowhere else.** It arrives as a
//!   [`crate::vaults::SecretValue`], which never prints itself, is handed to the core as a value
//!   wiped on drop, and is in no event, log or answer.
//! - **A chat whose command stopped waiting is nudged** ([`nudge`]): one line of purlis's own,
//!   naming the vault and key and never a value, sent as the person's message as the board allows
//!   (`crate::vaultroute::tell`, ADR 0064's gated send) and otherwise handed to its next turn.

use purlis_core::secrets::ask::{Answer, Answered, Named};
use purlis_core::secrets::links::{self, Target};
use purlis_core::secrets::{Ctx, Env, Zeroizing};

use crate::vaultroute::{Told, sent_as, tell};

/// **The person fills Secret ask `id`, or with no `value` says Not now** (#5068): a fill is
/// stored in the vault and every command that waits on it runs; Not now tells them it was
/// declined. Each chat that stopped waiting is nudged. A fill goes into `vault` under `key`
/// where the person chose either in place of the asked one (#5072), and the chats hear that
/// name.
#[tauri::command]
#[specta::specta]
pub async fn answer_secret_ask(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: u32,
    value: Option<crate::vaults::SecretValue>,
    vault: Option<String>,
    key: Option<String>,
    remember: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("answering a secret ask", move || {
        secret_answered(
            &held,
            id,
            value.map(crate::vaults::SecretValue::into_value),
            Named { vault, key },
            remember.as_deref(),
        )
    })
    .await
}

/// **The person fills Secret ask `id` with an entry they keep** (#5073): the asked key becomes a
/// Linked entry reading `key` in `vault`, never a copy, and every command that waits on it runs
/// with that entry's value. The chat hears filled and the key's name, never the entry.
#[tauri::command]
#[specta::specta]
pub async fn link_secret_ask(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: u32,
    vault: String,
    key: String,
    remember: Option<String>,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("linking a secret ask to an entry", move || {
        answered_with(
            &held,
            id,
            Answer::Link(Target { vault, key }),
            Named::as_asked(),
            remember.as_deref(),
        )
    })
    .await
}

/// **What the person pressed on an ask to allow a program** (ADR 0112).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RunGrantPress {
    /// One run of exactly this command.
    Once,
    /// This program with these variables and keys and any arguments, until the chat ends.
    Chat,
    /// As [`Self::Chat`], and write its line to the persona's tools (#5245).
    Always,
    /// Don't allow.
    Refuse,
}

/// **The person answers the ask to allow a program `id`** (ADR 0112, #5239): the run that waits
/// on it goes on, or hears it was not allowed, and a chat whose command stopped waiting is nudged.
#[tauri::command]
#[specta::specta]
pub async fn answer_run_grant(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: u32,
    press: RunGrantPress,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    crate::off_the_window("answering a run grant", move || {
        run_grant_answered(&held, network.as_ref(), id, press)
    })
    .await
}

/// [`answer_run_grant`]'s work.
pub fn run_grant_answered(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    id: u32,
    press: RunGrantPress,
) -> Result<(), String> {
    use purlis_core::secrets::ask::run::{Audited, RunAnswer};
    let answer = match press {
        RunGrantPress::Once => RunAnswer::Once,
        RunGrantPress::Chat => RunAnswer::ForChat,
        RunGrantPress::Always => RunAnswer::Always,
        RunGrantPress::Refuse => RunAnswer::Refuse,
    };
    let Some(open) = held
        .secret_asks()
        .open_runs()
        .into_iter()
        .find(|open| open.id == id)
    else {
        return Err("This ask has already been answered, or its chat has closed.".to_owned());
    };
    // Policy holds here as it holds at the run: it may have come to lock this since it was shown.
    if answer != RunAnswer::Refuse {
        use purlis_core::sandbox::grant::Level;
        // Before any audit: nothing was given. The core holds to it again as it answers.
        if answer != RunAnswer::Once && !open.bound.reviewed_for_any_arguments() {
            return Err(format!(
                "{} is allowed one exact command at a time: purlis has not reviewed it for any \
                 arguments. Press Allow once, or declare it as a tool.",
                open.bound.program_name()
            ));
        }
        let locks = purlis_core::sandbox::policy::Locks::of(held.root());
        if let Some(why) = locks.refuses_run_grant(&open.persona, &open.bound.program_name()) {
            return Err(why);
        }
        let level = match answer {
            RunAnswer::Once => Level::Once,
            RunAnswer::Always => Level::Project,
            RunAnswer::ForChat | RunAnswer::Refuse => Level::Chat,
        };
        if let Some(why) = locks.scope_removed(level) {
            return Err(why);
        }
        if open.not_its_vault
            && let Some(why) = locks.vault_grants_refused()
        {
            return Err(why);
        }
    }
    let audited = held.hooks().record_run_grant(
        held.root(),
        open.bound.chat,
        &Audited::granted(&open.bound, answer),
    );
    // Nothing is granted unrecorded. A Don't allow stands either way: it gives nothing.
    match (answer, audited) {
        (_, Ok(())) => {}
        (RunAnswer::Refuse, Err(why)) => {
            tracing::warn!("purlis: a run grant's Don't allow was not audited ({why})");
        }
        (_, Err(why)) => return Err(why),
    }
    // Written before any grant: a write that fails says why, the ask stays open, and nothing is
    // given. Where the person had allowed all of the persona's list, the Allow moves with the line.
    if answer == RunAnswer::Always {
        let written = match declare_the_tool(held, &open) {
            Ok(written) => written,
            Err(why) => {
                let taken_back = Audited::revoked_always(&open.bound);
                if let Err(unrecorded) =
                    held.hooks()
                        .record_run_grant(held.root(), open.bound.chat, &taken_back)
                {
                    tracing::warn!("purlis: a failed Always allow was not audited ({unrecorded})");
                }
                return Err(format!(
                    "purlis could not declare the tool, so nothing was allowed: {why}"
                ));
            }
        };
        // Always allow alone gives the persona the vault, audited as any such Allow is. The line
        // is written by now, so a failure here leaves the usual refused vault's Notice to ask.
        if let Some(grant) = written.vault
            && let Err(why) = give_the_vault(held, network, open.bound.chat, &grant)
        {
            tracing::warn!("purlis: Always allow did not give the persona the vault ({why})");
        }
        if let Some(shown) = written.allow
            && let Err(why) = crate::sandboxing::allow_shown(
                held.root(),
                &shown,
                &held.audit_then_record(network),
                crate::sandboxing::now_secs(),
            )
        {
            // The line is written and the chat runs on its Run grant; the usual Notice asks.
            tracing::warn!("purlis: the Allow did not move with the line ({why})");
        }
    }
    let answered = held
        .secret_asks()
        .answer_run(id, answer, crate::asking::the_window());
    held.chats().asks_may_have_moved();
    let answered = answered?;
    told_each(held, &answered.nudge, &answered.told);
    Ok(())
}

/// **Writes the line Always allow showed to the persona's tools** (#5245).
fn declare_the_tool(
    held: &crate::planes::Held,
    open: &purlis_core::secrets::ask::run::OpenRun,
) -> Result<purlis_core::settings::always_allow::Written, String> {
    purlis_core::settings::always_allow::write(held.root(), &open.persona, &open.bound)
}

/// **Gives the persona the vault Always allow named** (ADR 0112), audited first.
fn give_the_vault(
    held: &crate::planes::Held,
    network: Option<&purlis_core::sandboxblock::record::Record>,
    chat: u32,
    grant: &purlis_core::sandbox::local::VaultGrant,
) -> Result<(), String> {
    use purlis_core::sandbox::{grant::Audited, grant::Level, local};
    held.audit_then_record(network)(
        Some(chat),
        &Audited {
            granted: true,
            what: local::VAULT,
            target: &grant.target(),
            level: Level::You,
        },
    )?;
    local::grant_vault(held.root(), &grant.vault, &grant.persona).map_err(|why| why.to_string())
}

/// **One vault's entries a Secret ask's picker offers** (#5073): the vault, the keys a link may
/// read in it, and why none could be listed, where that is so. Names only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct EntriesOf {
    pub vault: String,
    pub keys: Vec<String>,
    /// Why its keys could not be listed: the provider's own first line.
    #[specta(optional)]
    pub unlisted: Option<String>,
}

/// **The entries the person could fill Secret ask `id` with** (#5073), vault by vault: every
/// key of every vault that holds a value of its own, but the asked one. Names only.
#[tauri::command]
#[specta::specta]
pub async fn secret_ask_entries(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: u32,
) -> Result<Vec<EntriesOf>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("listing the entries a secret ask may link to", move || {
        entries_for(&held, id)
    })
    .await
}

/// [`secret_ask_entries`]'s work.
pub fn entries_for(held: &crate::planes::Held, id: u32) -> Result<Vec<EntriesOf>, String> {
    let asked = held
        .secret_asks()
        .open()
        .into_iter()
        .find(|open| open.id == id)
        .map(|open| Target {
            vault: open.vault,
            key: open.key,
        })
        .ok_or_else(|| format!("secret ask {id} is no longer open"))?;
    let ctx = Ctx::new(held.root(), Env::from_process());
    Ok(links::pickable(&ctx, Some(&asked))
        .map_err(|e| e.message.to_string())?
        .into_iter()
        .map(|one| match one.keys {
            Ok(keys) => EntriesOf {
                vault: one.vault,
                keys,
                unlisted: None,
            },
            Err(why) => EntriesOf {
                vault: one.vault,
                keys: Vec::new(),
                unlisted: Some(why),
            },
        })
        .collect())
}

/// [`answer_secret_ask`]'s work.
pub fn secret_answered(
    held: &crate::planes::Held,
    id: u32,
    value: Option<Zeroizing<String>>,
    named: Named,
    remember: Option<&str>,
) -> Result<(), String> {
    answered_with(
        held,
        id,
        value.map_or(Answer::Decline, Answer::Fill),
        named,
        remember,
    )
}

/// The person's `answer` to Secret ask `id`, stored as `named`, given to the core as the
/// window's, and each chat that stopped waiting nudged. `remember` is the purpose of the line
/// the person ticked Remember for and read, written once the value is stored (#5240).
fn answered_with(
    held: &crate::planes::Held,
    id: u32,
    answer: Answer,
    named: Named,
    remember: Option<&str>,
) -> Result<(), String> {
    // Before anything is stored, while the row can still say why: after the Fill it is gone.
    if remember.is_some()
        && matches!(answer, Answer::Fill(_) | Answer::Link(_))
        && let Some(open) = held
            .secret_asks()
            .open()
            .into_iter()
            .find(|open| open.id == id)
    {
        crate::proposals::need_writable(held.root(), &open.persona)?;
    }
    let ctx = Ctx::new(held.root(), Env::from_process());
    let answered =
        held.secret_asks()
            .answer_as(&ctx, id, answer, named, crate::asking::the_window());
    held.chats().asks_may_have_moved();
    let answered = answered?;
    tracing::info!(
        "purlis: the person {} {}/{}",
        if answered.filled {
            "filled"
        } else {
            "declined"
        },
        answered.vault,
        answered.key,
    );
    nudge(held, &answered);
    crate::proposals::fill_remembered(held, &answered, remember)
}

/// Tells each chat [`Answered::nudge`] names [`Answered::told`]'s one line, as its board
/// allows: now, at its turn's end, or else at its next turn's start.
fn nudge(held: &crate::planes::Held, answered: &Answered) {
    told_each(held, &answered.nudge, &answered.told);
}

/// Tells each of `chats` `outcome`'s one line, as its board allows.
fn told_each(
    held: &crate::planes::Held,
    chats: &[u32],
    outcome: &purlis_core::askoutcome::Outcome,
) {
    for &chat in chats {
        let told = tell(
            held.vault_refusals(),
            chat,
            || {
                let glance = held.board().glance(chat);
                (glance.state, glance.asking)
            },
            outcome.line(),
            |chat, text| held.type_in_turn(chat, sent_as(text).as_bytes()),
        );
        if told == Told::NotSent {
            held.chats().tell_outcome(chat, outcome.clone());
        }
    }
}

/// **A chat's `secret_ask` line** (#5072, ADR 0109): `purlis secret ask` or the `secret_ask`
/// tool. The persona and the harness are this app's record of the chat whose token the line
/// carries; the rest is [`purlis_core::secrets::ask::raised`]'s. Answered once the person
/// answers, or pending at the chat's harness's timeout margin.
pub fn asked(
    held: &crate::planes::Held,
    asked: &purlis_core::secrets::ask::SecretAsk,
) -> purlis_core::hookwire::Answer {
    use purlis_core::hookwire::Answer;
    let Some(open) = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == asked.chat)
    else {
        return Answer::No {
            why: format!("chat {} is not one this app has open", asked.chat),
        };
    };
    let ctx = Ctx::new(held.root(), Env::from_process());
    let chat = purlis_core::secrets::ask::Chat {
        chat: asked.chat,
        persona: open.persona.as_deref(),
        harness: open.harness,
    };
    match purlis_core::secrets::ask::raised(&ctx, held.secret_asks(), chat, asked, &|| {
        held.chats().asks_may_have_moved();
    }) {
        Ok(said) => Answer::Secret(said),
        Err(why) => Answer::No { why },
    }
}
