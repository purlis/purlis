// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Provisioning is the person's press, in the window** (ADR 0128 D5).
//!
//! Signing in, the request to an organization's owners, SAML reauthorization, registering a
//! host's application, pinning its client id (*Use this application*), choosing which account
//! serves a project, trusting a clone and the second token's sign-in for CI files happen **only
//! on the person's press in the window**, never from a chat, the CLI or a socket call. Each of
//! those acts in this crate takes a [`HostScope`] ([`super::accounts::sign_in_with_grant`],
//! [`super::registered::use_application`], [`super::confirmed::confirm`],
//! [`super::confirmed::choose_account`], [`super::accounts::workflow::keep`] and the rest), and a
//! scope can only be claimed outside a chat's process tree ([`HostScope::claim`]).
//!
//! [`on_the_persons_press`] is where the window claims it. A chat, a `purlis` a chat runs, and the
//! `purlis` binary never do: `tests/it/provisioning_is_the_persons_press_in_the_window_only.rs`
//! reads their sources for it, and checks that every command of the window that provisions is in
//! the list no link serves.

use super::route::HostScope;

/// What a refused caller is told. It says what the act needs and where it is done, in the window.
pub const REFUSAL: &str = "Signing in, choosing an account, trusting a clone and setting up a \
    host's application are done with a press in purlis's window, never from a chat, the command \
    line or a socket. Open purlis and do it there.";

/// The window's scope for a provisioning act, or [`REFUSAL`]: refused inside a chat's process
/// tree, which is where every non-window caller runs.
pub fn on_the_persons_press() -> Result<HostScope, String> {
    HostScope::claim().ok_or_else(|| REFUSAL.to_string())
}

/// The commands of the window that provision a credential, a registration, a pin, a trust or an
/// account choice (D5). Every one is **window-only**: none is served on a link, and none is in
/// the command line. `session-protocol`'s own test holds this list to its list of commands no
/// link serves.
pub const PROVISIONING_COMMANDS: &[&str] = &[
    "forge_sign_out",
    "forge_sign_in_with_token",
    "forge_import_cli_login",
    "forge_device_sign_in_start",
    "forge_device_sign_in_wait",
    "forge_device_sign_in_cancel",
    "forge_signing_permission_start",
    "forge_signing_permission_wait",
    "forge_pkce_sign_in",
    "forge_pkce_sign_in_cancel",
    "forge_use_application",
    "forge_remove_application",
    "forge_choose_project_account",
    "forge_trust_clone_repo",
    "forge_ci_permission_start",
    "forge_ci_permission_wait",
    "forge_ci_permission_cancel",
    "vouch_clone",
    "allow_chat_push",
];

/// The entry points of this crate that provision, by name: what a source that is not the window's
/// must never call. A name is held to its callers by `tests/it/provisioning_is_the_persons_press_in_the_window_only.rs`.
pub const PROVISIONING_ENTRY_POINTS: &[&str] = &[
    "sign_in_with_grant(",
    "sign_in_with_token(",
    "pin_with_owner(",
    "registered::pin(",
    "registered::unpin(",
    "confirmed::confirm(",
    "record_clone(",
    "confirm_trusted(",
    "choose_account(",
    "forget_clone(",
    "forget_by_key(",
    "record_unchosen(",
    "headless_repos(",
    "ChatRoute::enrol(",
    "workflow::keep(",
    "workflow::begin(",
    "with_forge_id(",
    "ForgeProxy::enrol(",
    "for_ci_push(",
    "vouch_clone(",
    "trust_this_clone(",
    "let_chat_push(",
    "on_the_persons_press(",
    "HostScope::claim(",
];
