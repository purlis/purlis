// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Phase 1a's Window copy that core writes, every line by its W-number** (T1a-75, #6755): core's
//! half of the copy module, for the sentences core writes for the window: refusals at a turn's
//! end or a start, Notices, Ask bodies and the Blocked log line. The lines are
//! `app/src/windowCopy.table.md`'s, the table the operator approved, word for word; the window's
//! half is `app/src/windowCopy.ts`, and `app/src/windowCopy.test.ts` holds the table and both
//! halves to each other. Core never builds one of these sentences inline: it takes the line from
//! [`line`] and fills its slots with [`fill`]. `app/src/copy.rust.test.ts` holds every line to
//! the copy guide's rules.
//!
//! A slot is written as the table writes it, `<pool>`; a line with several forms has a const per
//! form, named as the window's half names it (`W16_LATER` is `windowCopy.W16.later`). The three
//! table-wide rules are [`on_this_os`] and [`on_pool`].

/// Every line core writes, as the table words it.
pub mod line {
    /// W10: Ask, New chat; Needs you.
    pub const W10: &str = "Your keychain is locked. Unlock it to start this chat.";
    /// W11: Refusal, the turn's end.
    pub const W11: &str =
        "purlis: your Anthropic key was removed. Add it again, then send your prompt again.";
    /// W12: Ask, Needs you.
    pub const W12_MANY: &str = "Your Anthropic key was removed. Add it again to use these chats: <chats>. The conversation is kept. Don't use `/login` here.";
    pub const W12_ONE: &str = "Your Anthropic key was removed. Add it again to use <chat>. The conversation is kept. Don't use `/login` here.";
    /// W13: Ask, Needs you.
    pub const W13: &str = "Your Anthropic key was changed outside purlis. Mark it again to use it. The conversation is kept. Don't use `/login` here.";
    /// W15: Notice, Inbox.
    pub const W15: &str =
        "Anthropic refused your key. Replace it, or start a new chat. Don't use `/login` here.";
    /// W16: Notice, Inbox.
    pub const W16_AT: &str =
        "Anthropic is limiting requests on your Anthropic account. Try again after <time>.";
    pub const W16_LATER: &str =
        "Anthropic is limiting requests on your Anthropic account. Try again later.";
    /// W17: Notice, Inbox.
    pub const W17: &str = "Anthropic isn't answering. Try again, or start a new chat.";
    /// W18: Notice, Inbox.
    pub const W18: &str = "This conversation is too long for the model. Compact it in Claude Code, or start a new chat.";
    /// W19: Notice, Inbox.
    pub const W19_ANSWERED: &str = "Anthropic answered <status>: <message>. Start a new chat.";
    pub const W19_PERMISSION: &str = "Anthropic says this key can't do that: <message>.";
    /// W20: Notice, Inbox.
    pub const W20_HERE: &str = "purlis can't reach Anthropic through this network. purlis uses this computer's proxy settings and trusted certificates.";
    pub const W20_RUNNER: &str = "Pool <name> can't reach Anthropic through its network.";
    /// W21: Notice, Inbox.
    pub const W21_HERE: &str = "Anthropic's certificate isn't trusted on this computer. purlis uses this computer's proxy settings and trusted certificates.";
    pub const W21_RUNNER: &str = "Anthropic's certificate isn't trusted on pool <name>.";
    /// W22: Notice, the pane's band (once per run).
    pub const W22: &str =
        "This chat runs on your Anthropic key. Signing in to a Claude plan here does nothing.";
    /// W23: Notice, the pane's band.
    pub const W23: &str = "Claude Code <version> didn't use the route purlis set";
    /// W25: Refusal, the turn's end.
    pub const W25: &str = "purlis blocked this request: <reason>.";
    /// W26: Refusal, the turn's end.
    pub const W26: &str = "purlis: <lock> now blocks this.";
    /// W26a: Notice, Inbox.
    pub const W26A: &str = "<lock> now blocks this chat's requests.";
    /// W26b: Refusal, New chat, the start.
    pub const W26B: &str =
        "<lock> blocks <choice>. Nothing else can start this chat until the policy allows it.";
    /// W27: Refusal, the card, the start.
    pub const W27: &str = "Only an Anthropic key works in this version";
    /// W28: Ask, card, Needs you.
    pub const W28: &str = "This profile's Anthropic key is in the vault <name>. Copy it to your keys to use it in any project?";
    /// W29: Refusal, New chat.
    pub const W29: &str =
        "Set by your administrator in <file>: <key>, which a chat on a key can't use.";
    /// W29a: Refusal, New chat.
    pub const W29A: &str = "<file> sets <key>, which a chat on a key can't use.";
    /// W30: Ask, above the footer; Needs you (the pool key allowance).
    pub const W30: &str = "Your Anthropic key will be sent to pool <name> for its chats.";
    /// W34: Notice, Inbox.
    pub const W34: &str = "The link to pool <name> dropped. Its chat keeps your key until it ends.";
    /// W35: Notice, Inbox.
    pub const W35: &str = "Pool <name>'s host restarted. Its chat ended.";
    /// W36: Refusal, New chat.
    pub const W36: &str = "<file> requires purlis <version> or later (`min-version`). Chats can't start. Update purlis.";
    /// W38: Notice, Inbox.
    pub const W38: &str = "This chat has 64 model calls running at once. Wait for them to finish, then send your prompt again.";
    /// W39: Refusal, New chat.
    pub const W39: &str =
        "purlis can't use a key approval setting other than off in this version: <setting>.";
    /// W40: Refusal, New chat.
    pub const W40_ONE_PASSWORD: &str = "purlis can't use a key from 1Password in this version.";
    pub const W40_HASHI_CORP_VAULT: &str =
        "purlis can't use a key from HashiCorp Vault in this version.";
    /// W41: Refusal, New chat.
    pub const W41: &str = "purlis can't use a cloud sign-in in this version: <cloud>.";
    /// W42: Refusal, New chat.
    pub const W42: &str = "purlis can't use a local model server in this version: <server>.";
    /// W43: Refusal, New chat.
    pub const W43: &str =
        "purlis can't use a project's model provider file in this version: <file>.";
    /// W44: Refusal, New chat.
    pub const W44: &str = "purlis can't use a chosen model in this version: <file>.";
    /// W56: Refusal, the card, the start.
    pub const W56: &str = "Add your key again in Settings › You › Profiles";
    /// W59: Refusal, the turn's end.
    pub const W59: &str = "purlis: your Anthropic key was changed outside purlis. Mark it again, then send your prompt again.";
    /// W62: Notice, the pane's band.
    pub const W62: &str =
        "purlis couldn't reopen this chat's connection to Anthropic, so its run ended.";
    /// W63: Refusal, New chat.
    pub const W63: &str = "purlis can't start this chat: no local port could be opened for it.";
    /// W64: Refusal, New chat.
    pub const W64: &str =
        "<file> can't be read: <error>. Chats on a key can't start until it's fixed.";
    /// W65: Refusal, the turn's end.
    pub const W65: &str = "purlis: <file> can't be read (<error>), so this request was blocked.";
    /// W66: Refusal, New chat.
    pub const W66: &str = "<file> sets `models.<key>`, which this version of purlis can't enforce. Chats can't start.";
    /// W67: Refusal, the turn's end.
    pub const W67: &str = "purlis: this chat waited 15 minutes for your computer, so the request was blocked. Open purlis on your computer, then send your prompt again.";
    /// W70: Refusal, the move.
    pub const W70: &str =
        "This chat can't move to pool <name>: its key route can't run there. <what would work>.";
    /// W78: Refusal, the turn's end; a refused start.
    pub const W78_TURN_END: &str =
        "purlis: this key is for Anthropic and can't be sent to <origin>.";
    pub const W78_START: &str = "This key is for Anthropic and can't be sent to <origin>.";
    /// W80: Refusal, the turn's end (part).
    pub const W80_NO_RUN_TOKEN: &str = "it carried no run token";
    pub const W80_NOT_A_MODEL_CALL: &str = "<method> <path> isn't a model call";
    pub const W80_MODEL_UNREADABLE: &str = "its model couldn't be read";
    pub const W80_REPEATED_FIELD: &str = "it repeated a field";
    pub const W80_ENCODED_BODY: &str = "its body was encoded";
    pub const W80_BODY_UNREADABLE: &str = "its body couldn't be read whole";
    pub const W80_CLAUDE_PLAN_SIGN_IN: &str = "it carried a Claude plan sign-in";
    pub const W80_FROM_A_WEB_PAGE: &str = "it came from a web page";
    pub const W80_BODY_OVER128_MB: &str = "its body is over 128 MB";
    pub const W80_PLAN_SIGN_IN: &str = "it carried a plan sign-in";
    /// W81: Refusal, printed by Claude Code when it gives up (transport).
    pub const W81_HOST_NOT_FOUND: &str = "purlis couldn't find <host>.";
    pub const W81_NO_CONNECTION: &str = "purlis couldn't connect to <host>.";
    pub const W81_UNTRUSTED_CERTIFICATE: &str = "<host>'s certificate isn't trusted.";
    pub const W81_PROXY_REFUSED: &str = "The outbound proxy refused the connection.";
    pub const W81_NO_ANSWER: &str = "<host> didn't answer within <n> seconds.";
    pub const W81_RESET: &str = "<host> reset the connection.";
    /// W82: Refusal, printed by Claude Code when it gives up (the cap's 429).
    pub const W82: &str = "purlis: this chat has 64 model calls running at once.";
    /// W83: Refusal, printed by Claude Code when it gives up (the body-wait 429).
    pub const W83: &str = "purlis: waiting for room to hold this request.";
    /// W85: Notice, Inbox.
    pub const W85: &str = "Your Anthropic account has reached its spend limit: <message>. Raise it at Anthropic, then send your prompt again.";
    /// W86: Notice, Inbox.
    pub const W86: &str = "Anthropic reports a billing problem on your Anthropic account: <message>. Fix it at Anthropic, then send your prompt again.";
    /// W87: Notice, Inbox.
    pub const W87: &str = "Anthropic has paused requests on your Anthropic account until its usage limit resets: <message>.";
    /// W92: Refusal, the start.
    pub const W92: &str = "A local harness declaration can't set `route.<field>` in this version.";
    /// W93: Notice, the pane's band (once per run).
    pub const W93: &str =
        "This chat runs on your Anthropic key. Signing in to another plan here does nothing.";
    /// W98: Refusal, the port grant form's error state.
    pub const W98: &str = "Port <port> is purlis's own key proxy, so no chat can be granted it.";
    /// W99: Row, the Blocked log, `network.rs`'s words for `Source::KeyProxy`.
    pub const W99: &str = "blocked by purlis's key proxy";
    /// W101: Refusal, where refused declarations are shown (`Declarations::refused`) (reason).
    pub const W101: &str = "`charter.local.toml` sets `[sign_in] plans` for <harness>, which only purlis's own declaration may set. purlis uses its own.";
    /// W105: Refusal, Add profile's form, under *A key* shown off for another harness; the card of an old non-Claude-Code profile holding an Anthropic key.
    pub const W105: &str = "Only Claude Code can use a key in this version.";
    /// W106: Refusal, New chat.
    pub const W106: &str = "<file> on <pool> requires purlis <version> or later (`min-version`). Chats can't start there.";
    /// W107: Refusal, New chat.
    pub const W107: &str = "<file> on <pool> can't be read: <error>. Chats on a key can't start there until it's fixed.";
    /// W108: Refusal, the turn's end.
    pub const W108: &str =
        "purlis: <file> on <pool> can't be read (<error>), so this request was blocked.";
    /// W109: Refusal, the turn's end.
    pub const W109: &str =
        "purlis: <file> requires purlis <version> or later, so this request was blocked.";
    /// W110: Refusal, the turn's end.
    pub const W110: &str = "purlis: <lock> on <pool> now blocks this.";
    /// W111: Notice, Inbox.
    pub const W111: &str = "Anthropic refused the key this chat held. purlis read your key again. Send your prompt again.";
    /// W112: Refusal, New chat's footer.
    pub const W112: &str = "purlis couldn't save sending your Anthropic key to <pool>: <error>. The chat didn't start.";
    /// W117: Refusal, New chat.
    pub const W117: &str = "Chats on a key start in a later update of this build.";
    /// W118: Refusal, New chat.
    pub const W118: &str = "This profile's key is stored the old way. A later update of this build copies it to your keys.";
    /// W119: Refusal, New chat.
    pub const W119: &str = "<file> is shared with this project's other members and sets <key>, which a chat on a key can't use.";
    /// W130: Refusal, New chat.
    pub const W130: &str = "<file> on <pool> sets `models.<key>`, which this version of purlis can't enforce. Chats can't start there.";
    /// W131: Refusal, the turn's end.
    pub const W131: &str = "purlis: <file> on <pool> sets `models.<key>`, which this version of purlis can't enforce, so this request was blocked.";
}

/// The systems purlis runs on, for the table's first two rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    /// The system this build runs on.
    pub const HERE: Os = if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
}

/// `line` with each `<slot>` filled from `slots`, in one pass, so a value is never read as a
/// slot. A slot `slots` does not name is left as it is.
pub fn fill(line: &str, slots: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let value = after.find('>').and_then(|close| {
            let name = &after[1..close];
            slots
                .iter()
                .find(|(slot, _)| *slot == name)
                .map(|(_, value)| (close, *value))
        });
        match value {
            Some((close, value)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('<');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// **A line as `os` says it**, the table's first two rules: on macOS *computer* reads *Mac*, and
/// off macOS *your keychain* reads *your system keychain*.
pub fn on_this_os(line: &str, os: Os) -> String {
    match os {
        Os::MacOs => line.replace("computer", "Mac"),
        Os::Linux | Os::Windows => line.replace("our keychain", "our system keychain"),
    }
}

/// **The thing a runner chat's line names, on its pool**, the table's third rule: *<lock> on gpu*
/// in a runner chat's Notice or refusal, and the thing alone off a runner (`pool` is `None`).
pub fn on_pool(thing: &str, pool: Option<&str>) -> String {
    match pool {
        Some(pool) => format!("{thing} on {pool}"),
        None => thing.to_owned(),
    }
}

#[cfg(test)]
mod tests;
