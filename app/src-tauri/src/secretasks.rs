// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Secret ask, answered in the window** (#5068, #5071, ADR 0109): the window's half of
//! [`purlis_core::secrets::ask`].
//!
//! A chat's brokered `secret exec` whose vault holds no value under a key it hands over waits on
//! a Secret ask (`crate::vaults::run_brokered`), listed with every other ask
//! (`crate::asking::secret_ask`). The person fills it, with a value typed into the ask's masked
//! field, or chooses Not now ([`answer_secret_ask`]).
//!
//! - **Only the window answers.** [`answer_secret_ask`] is a Tauri command of the window's,
//!   never served on the link to `charterd` (`purlis_session_protocol::ui::WINDOW_ONLY`), and it
//!   answers as the window's own scope ([`crate::asking::the_window`]), the one that holds
//!   `AnswerASecret`. The core refuses any other answerer.
//! - **The value goes to the vault's provider and nowhere else.** It arrives as a
//!   [`crate::vaults::SecretValue`], which never prints itself, is handed to the core as a value
//!   wiped on drop, and is in no event, log or answer.
//! - **A chat whose command stopped waiting is nudged** ([`nudge`]): one line of purlis's own,
//!   naming the vault and key and never a value, sent as the person's message as the board allows
//!   (`crate::vaultroute::tell`, ADR 0064's gated send) and otherwise handed to its next turn.

use purlis_core::secrets::ask::{Answer, Answered, Named};
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
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("answering a secret ask", move || {
        secret_answered(
            &held,
            id,
            value.map(crate::vaults::SecretValue::into_value),
            Named { vault, key },
        )
    })
    .await
}

/// [`answer_secret_ask`]'s work.
pub fn secret_answered(
    held: &crate::planes::Held,
    id: u32,
    value: Option<Zeroizing<String>>,
    named: Named,
) -> Result<(), String> {
    let answer = value.map_or(Answer::Decline, Answer::Fill);
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
    Ok(())
}

/// Tells each chat [`Answered::nudge`] names [`Answered::told`]'s one line, as its board
/// allows: now, at its turn's end, or else at its next turn's start.
fn nudge(held: &crate::planes::Held, answered: &Answered) {
    for &chat in &answered.nudge {
        let told = tell(
            held.vault_refusals(),
            chat,
            || {
                let glance = held.board().glance(chat);
                (glance.state, glance.asking)
            },
            answered.told.line(),
            |chat, text| held.type_in_turn(chat, sent_as(text).as_bytes()),
        );
        if told == Told::NotSent {
            held.chats().tell_outcome(chat, answered.told.clone());
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
