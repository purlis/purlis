// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Phase 1a's Window copy, every line by its W-number** (T1a-75, #6755): the window's half of
 * the copy module. The lines are `windowCopy.table.md`'s, the table the operator approved, word
 * for word; core's half is `crates/purlis-core/src/window_copy.rs`, and `windowCopy.test.ts`
 * holds the table and both halves to each other. A line takes its words from here and nowhere
 * else, and a change to one is a change to the table first.
 *
 * A line with a slot is a function of its slots, named as the table writes them (`<pool>` is
 * `pool`, `<what would work>` is `whatWouldWork`); a line with several forms is a record of them.
 * A line only core says (at a turn's end, printed by Claude Code, the Blocked log) is core's
 * alone, so it is not here. The three table-wide rules are `onThisOs` and `onPool`, applied by
 * whoever draws the line.
 */
import { counted } from "./dispatches";

/** `one` for a count of exactly one, `many` for every other: a word that agrees with a count. */
const agreeing = (count: number, one: string, many: string) => (count === 1 ? one : many);

/** W5, through `counted`: Remove's confirmation, with how many chats use the key. */
function COUNTED_W5({ count }: { count: number }): string {
  if (count === 0) return "No chats use this key.";
  return `${counted(count, "chat", "chats")} ${agreeing(count, "uses", "use")} this key. ${agreeing(count, "Its", "Their")} next request is blocked, and the conversation is kept.`;
}

/** W123, through `counted`: Stop sending's confirmation, with how many chats on the pool use it. */
function COUNTED_W123({ count, pool }: { count: number; pool: string }): string {
  const asked = `Stop sending this key to ${pool}?`;
  if (count === 0) return `${asked} No chats there use it.`;
  return `${asked} ${counted(count, "chat", "chats")} there ${agreeing(count, "stops", "stop")} at ${agreeing(count, "its", "their")} next request, and ${agreeing(count, "its conversation is", "their conversations are")} kept.`;
}

export const windowCopy = {
  /** W1: Label, J0's form, the key form. */
  W1: "Paste your Anthropic key",
  /** W2: Label, J0's form, the key form, *Copy to your keys*. */
  W2: "Use the key you already have",
  /** W3: Label, J0's form (pre-filled name). */
  W3: "claude-anthropic",
  /** W4: Row, Your keys (empty state). */
  W4: "No keys yet",
  /** W5: Ask, Your keys (Remove's confirmation). */
  W5: COUNTED_W5,
  /** W6: Row, Your keys. */
  W6: "This key can't be read. Unlock your keychain to use it.",
  /** W7: Row, J0's form, the key form, *Replace*, Your keys. */
  W7: "Not checked yet. purlis couldn't check this key with Anthropic.",
  /** W8: Row, J0's form, the key form, *Replace*. */
  W8: "Anthropic didn't accept this key. Check it, or save it anyway.",
  /** W9: Row, J0's form, the key form. */
  W9: "You already have an Anthropic key. Replace it?",
  /** W10: Ask, New chat; Needs you. */
  W10: "Your keychain is locked. Unlock it to start this chat.",
  /** W12: Ask, Needs you. */
  W12: {
    many: ({ chats }: { chats: string }) =>
      `Your Anthropic key was removed. Add it again to use these chats: ${chats}. The conversation is kept. Don't use \`/login\` here.`,
    one: ({ chat }: { chat: string }) =>
      `Your Anthropic key was removed. Add it again to use ${chat}. The conversation is kept. Don't use \`/login\` here.`,
  },
  /** W13: Ask, Needs you. */
  W13: "Your Anthropic key was changed outside purlis. Mark it again to use it. The conversation is kept. Don't use `/login` here.",
  /** W14: Row, New chat's card. */
  W14: "Key missing",
  /** W15: Notice, Inbox. */
  W15: "Anthropic refused your key. Replace it, or start a new chat. Don't use `/login` here.",
  /** W16: Notice, Inbox. */
  W16: {
    at: ({ time }: { time: string }) =>
      `Anthropic is limiting requests on your Anthropic account. Try again after ${time}.`,
    later: "Anthropic is limiting requests on your Anthropic account. Try again later.",
  },
  /** W17: Notice, Inbox. */
  W17: "Anthropic isn't answering. Try again, or start a new chat.",
  /** W18: Notice, Inbox. */
  W18: "This conversation is too long for the model. Compact it in Claude Code, or start a new chat.",
  /** W19: Notice, Inbox. */
  W19: {
    answered: ({ status, message }: { status: string; message: string }) =>
      `Anthropic answered ${status}: ${message}. Start a new chat.`,
    permission: ({ message }: { message: string }) =>
      `Anthropic says this key can't do that: ${message}.`,
  },
  /** W20: Notice, Inbox. */
  W20: {
    here: "purlis can't reach Anthropic through this network. purlis uses this computer's proxy settings and trusted certificates.",
    runner: ({ name }: { name: string }) =>
      `Pool ${name} can't reach Anthropic through its network.`,
  },
  /** W21: Notice, Inbox. */
  W21: {
    here: "Anthropic's certificate isn't trusted on this computer. purlis uses this computer's proxy settings and trusted certificates.",
    runner: ({ name }: { name: string }) =>
      `Anthropic's certificate isn't trusted on pool ${name}.`,
  },
  /** W22: Notice, the pane's band (once per run). */
  W22: "This chat runs on your Anthropic key. Signing in to a Claude plan here does nothing.",
  /** W23: Notice, the pane's band. */
  W23: ({ version }: { version: string }) =>
    `Claude Code ${version} didn't use the route purlis set`,
  /** W24: Row, card, chat details (said once). */
  W24: "This project's Claude Code settings name another route. This chat uses purlis's.",
  /** W26a: Notice, Inbox. */
  W26a: ({ lock }: { lock: string }) => `${lock} now blocks this chat's requests.`,
  /** W26b: Refusal, New chat, the start. */
  W26b: ({ lock, choice }: { lock: string; choice: string }) =>
    `${lock} blocks ${choice}. Nothing else can start this chat until the policy allows it.`,
  /** W27: Refusal, the card, the start. */
  W27: "Only an Anthropic key works in this version",
  /** W28: Ask, card, Needs you. */
  W28: ({ name }: { name: string }) =>
    `This profile's Anthropic key is in the vault ${name}. Copy it to your keys to use it in any project?`,
  /** W29: Refusal, New chat. */
  W29: ({ file, key }: { file: string; key: string }) =>
    `Set by your administrator in ${file}: ${key}, which a chat on a key can't use.`,
  /** W29a: Refusal, New chat. */
  W29a: ({ file, key }: { file: string; key: string }) =>
    `${file} sets ${key}, which a chat on a key can't use.`,
  /** W30: Ask, above the footer; Needs you (the pool key allowance). */
  W30: ({ name }: { name: string }) =>
    `Your Anthropic key will be sent to pool ${name} for its chats.`,
  /** W31: Row, the *Runs on* row (said once). */
  W31: "Its admins can read a key held there for a run.",
  /** W32: Row, New chat (unavailable card). */
  W32: ({ pool }: { pool: string }) =>
    `purlis sends a key only to a runner marked as yours. ${pool} isn't marked.`,
  /** W33: Row, the *Runs on* row. */
  W33: ({ name }: { name: string }) =>
    `Pool ${name} runs an older purlis, so a key can't be sent there yet.`,
  /** W34: Notice, Inbox. */
  W34: ({ name }: { name: string }) =>
    `The link to pool ${name} dropped. Its chat keeps your key until it ends.`,
  /** W35: Notice, Inbox. */
  W35: ({ name }: { name: string }) => `Pool ${name}'s host restarted. Its chat ended.`,
  /** W36: Refusal, New chat. */
  W36: ({ file, version }: { file: string; version: string }) =>
    `${file} requires purlis ${version} or later (\`min-version\`). Chats can't start. Update purlis.`,
  /** W37: Row, harness card tab (spend words). */
  W37: {
    metered: "Metered",
    reportedByClaudeCode: "Reported by Claude Code",
    notReported: "Not reported",
    priceUnknown: "Price unknown",
    estimated: "Estimated",
    noCallsYet: "No calls yet",
    spendNotFullyReported: "Spend not fully reported",
  },
  /** W38: Notice, Inbox. */
  W38: "This chat has 64 model calls running at once. Wait for them to finish, then send your prompt again.",
  /** W39: Refusal, New chat. */
  W39: ({ setting }: { setting: string }) =>
    `purlis can't use a key approval setting other than off in this version: ${setting}.`,
  /** W40: Refusal, New chat. */
  W40: {
    onePassword: "purlis can't use a key from 1Password in this version.",
    hashiCorpVault: "purlis can't use a key from HashiCorp Vault in this version.",
  },
  /** W41: Refusal, New chat. */
  W41: ({ cloud }: { cloud: string }) =>
    `purlis can't use a cloud sign-in in this version: ${cloud}.`,
  /** W42: Refusal, New chat. */
  W42: ({ server }: { server: string }) =>
    `purlis can't use a local model server in this version: ${server}.`,
  /** W43: Refusal, New chat. */
  W43: ({ file }: { file: string }) =>
    `purlis can't use a project's model provider file in this version: ${file}.`,
  /** W44: Refusal, New chat. */
  W44: ({ file }: { file: string }) => `purlis can't use a chosen model in this version: ${file}.`,
  /** W45: Row, Inbox, under W20, W16, W17, W38; after *Try again* or an answered Ask it is an Update in the same place until the next call or a dismiss (a Notice line). */
  W45: "Send your prompt again.",
  /** W46: Row, Your keys. */
  W46: "Anthropic didn't accept this key when you saved it.",
  /** W47: Row, the *Runs on* row. */
  W47: ({ name }: { name: string }) => `Pool ${name} · Needs an update`,
  /** W48: Row, under W47. */
  W48: "Running chats keep going on the old runner",
  /** W49: Row, the *Runs on* row (busy). */
  W49: "Updating…",
  /** W50: Row, Your keys (drawn with W5's confirmation). */
  W50: ({ pool }: { pool: string }) =>
    `${pool} can't be reached. The chat there keeps this key until it ends or the link returns. To stop it now, revoke the key at Anthropic.`,
  /** W51: Row, the card. */
  W51: ({ who, file }: { who: string; file: string }) =>
    `Locked by policy, set by ${who} in ${file}.`,
  /** W52: Row, the card. */
  W52: ({ name, who, file }: { name: string; who: string; file: string }) =>
    `Locked by policy on pool ${name}, set by ${who} in ${file}.`,
  /** W53: Row, the card, chat details. */
  W53: "routed by your administrator's Claude Code settings",
  /** W54: Row, chat details. */
  W54: ({ source }: { source: string }) => `purlis can't read ${source}, so it wasn't checked.`,
  /** W55: Row, Your keys. */
  W55: "This key's entry is missing from your keychain.",
  /** W56: Refusal, the card, the start. */
  W56: "Add your key again in Settings › You › Profiles",
  /** W57: Row, under W28. */
  W57: "This is a Claude plan sign-in, not an API key, so purlis can't copy it to your keys.",
  /** W58: Row, harness card tab (spend line suffix). */
  W58: ({ time }: { time: string }) => `as of ${time}`,
  /** W60: Row, New chat's card, Your keys' row. */
  W60: "Key changed outside purlis",
  /** W61: Update, the pane's band. */
  W61: "purlis reopened this chat's connection to Anthropic.",
  /** W62: Notice, the pane's band. */
  W62: "purlis couldn't reopen this chat's connection to Anthropic, so its run ended.",
  /** W63: Refusal, New chat. */
  W63: "purlis can't start this chat: no local port could be opened for it.",
  /** W64: Refusal, New chat. */
  W64: ({ file, error }: { file: string; error: string }) =>
    `${file} can't be read: ${error}. Chats on a key can't start until it's fixed.`,
  /** W66: Refusal, New chat. */
  W66: ({ file, key }: { file: string; key: string }) =>
    `${file} sets \`models.${key}\`, which this version of purlis can't enforce. Chats can't start.`,
  /** W68: Row, the chat's card. */
  W68: "Waiting for your computer to link again before this chat can restart.",
  /** W69: Row, New chat. */
  W69: ({ card, name }: { card: string; name: string }) =>
    `${card} can't run on pool ${name}. Pick another card.`,
  /** W70: Refusal, the move. */
  W70: ({ name, whatWouldWork }: { name: string; whatWouldWork: string }) =>
    `This chat can't move to pool ${name}: its key route can't run there. ${whatWouldWork}.`,
  /** W71: Row, New chat (unavailable card). */
  W71: ({ name }: { name: string }) =>
    `purlis doesn't carry a Claude plan from this computer to pool ${name}. Use a profile signed in on the runner.`,
  /** W72: Row, the pool's switch. */
  W72: "Each vault value or token still asks you when it is sent. A model key is sent once you allow it for this pool.",
  /** W73: Row, under W47. */
  W73: ({ name, error }: { name: string; error: string }) =>
    `Pool ${name} couldn't update: ${error}.`,
  /** W74: Row, the *Runs on* row. */
  W74: ({ name }: { name: string }) =>
    `Pool ${name} runs your team's image. Update the image to send keys there.`,
  /** W75: Row, the *Runs on* row. */
  W75: ({
    name,
    runnerVersion,
    version,
  }: {
    name: string;
    runnerVersion: string;
    version: string;
  }) =>
    `Pool ${name} runs purlis ${runnerVersion}, which doesn't match this purlis (${version}). Update the pool to send keys there.`,
  /** W76: Row, the harness card tab's spend block. */
  W76: ({ chat }: { chat: string }) => `For ${chat}`,
  /** W77: Row, under W23. */
  W77: ({ version }: { version: string }) => `purlis last tested Claude Code ${version}.`,
  /** W78: Refusal, the turn's end; a refused start. */
  W78: {
    turnEnd: ({ origin }: { origin: string }) =>
      `purlis: this key is for Anthropic and can't be sent to ${origin}.`,
    start: ({ origin }: { origin: string }) =>
      `This key is for Anthropic and can't be sent to ${origin}.`,
  },
  /** W79: Row, the Network page. */
  W79: "Calls to model providers go through purlis's key proxy and show here as this chat's.",
  /** W84: Row, under W10. */
  W84: "purlis couldn't unlock your keychain.",
  /** W85: Notice, Inbox. */
  W85: ({ message }: { message: string }) =>
    `Your Anthropic account has reached its spend limit: ${message}. Raise it at Anthropic, then send your prompt again.`,
  /** W86: Notice, Inbox. */
  W86: ({ message }: { message: string }) =>
    `Anthropic reports a billing problem on your Anthropic account: ${message}. Fix it at Anthropic, then send your prompt again.`,
  /** W87: Notice, Inbox. */
  W87: ({ message }: { message: string }) =>
    `Anthropic has paused requests on your Anthropic account until its usage limit resets: ${message}.`,
  /** W88: Row, Your keys, the key form. */
  W88: ({ error }: { error: string }) => `purlis couldn't replace this key: ${error}.`,
  /** W89: Row, J0's form, the key form. */
  W89: ({ error }: { error: string }) => `purlis couldn't save this key: ${error}.`,
  /** W90: Row, You › Profiles. */
  W90: "Its key is missing from your keys",
  /** W91: Row, J0's form, the key form. */
  W91: ({ lock }: { lock: string }) =>
    `${lock} doesn't let purlis reach Anthropic to check this key.`,
  /** W92: Refusal, the start. */
  W92: ({ field }: { field: string }) =>
    `A local harness declaration can't set \`route.${field}\` in this version.`,
  /** W93: Notice, the pane's band (once per run). */
  W93: "This chat runs on your Anthropic key. Signing in to another plan here does nothing.",
  /** W94: Row, the Chat log, `ASK_SAID` `model_key`. */
  W94: "about a model key",
  /** W95: Row, the Chat log, `ASK_SAID` `pool_allow`. */
  W95: "about sending a key to a pool",
  /** W96: Label, the Notices view, `NOTICE_KINDS` (Notice kind, one / many). */
  W96: {
    one: "Model key",
    many: "Model keys",
  },
  /** W97: Label, the Notices view, `NOTICE_KINDS` (Notice kind, one / many). */
  W97: {
    one: "Model call cap",
    many: "Model call caps",
  },
  /** W98: Refusal, the port grant form's error state. */
  W98: ({ port }: { port: string }) =>
    `Port ${port} is purlis's own key proxy, so no chat can be granted it.`,
  /** W100: Row, after W51, on a refused card or row (a second sentence). */
  W100: ({ lock }: { lock: string }) => `The lock is ${lock}.`,
  /** W101: Refusal, where refused declarations are shown (`Declarations::refused`) (reason). */
  W101: ({ harness }: { harness: string }) =>
    `\`charter.local.toml\` sets \`[sign_in] plans\` for ${harness}, which only purlis's own declaration may set. purlis uses its own.`,
  /** W102: Label, Add profile's form. */
  W102: "Signs in with",
  /** W103: Label, Add profile's form. */
  W103: "Its own sign-in",
  /** W104: Label, Add profile's form. */
  W104: "A key",
  /** W105: Refusal, Add profile's form, under *A key* shown off for another harness; the card of an old non-Claude-Code profile holding an Anthropic key. */
  W105: "Only Claude Code can use a key in this version.",
  /** W106: Refusal, New chat. */
  W106: ({ file, pool, version }: { file: string; pool: string; version: string }) =>
    `${file} on ${pool} requires purlis ${version} or later (\`min-version\`). Chats can't start there.`,
  /** W107: Refusal, New chat. */
  W107: ({ file, pool, error }: { file: string; pool: string; error: string }) =>
    `${file} on ${pool} can't be read: ${error}. Chats on a key can't start there until it's fixed.`,
  /** W111: Notice, Inbox. */
  W111: "Anthropic refused the key this chat held. purlis read your key again. Send your prompt again.",
  /** W112: Refusal, New chat's footer. */
  W112: ({ pool, error }: { pool: string; error: string }) =>
    `purlis couldn't save sending your Anthropic key to ${pool}: ${error}. The chat didn't start.`,
  /** W113: Update, New chat's footer. */
  W113: ({ pool }: { pool: string }) => `Sent your Anthropic key to ${pool} and started the chat.`,
  /** W114: Row, Your keys. */
  W114: ({ error }: { error: string }) =>
    `purlis couldn't remove this key: ${error}. It is still in your keys.`,
  /** W115: Row, Your keys. */
  W115: ({ error }: { error: string }) => `Your keys can't be read: ${error}.`,
  /** W116: Row, New chat's card. */
  W116: ({ pool }: { pool: string }) => `${pool} can't be reached right now.`,
  /** W117: Refusal, New chat. */
  W117: "Chats on a key start in a later update of this build.",
  /** W118: Refusal, New chat. */
  W118: "This profile's key is stored the old way. A later update of this build copies it to your keys.",
  /** W119: Refusal, New chat. */
  W119: ({ file, key }: { file: string; key: string }) =>
    `${file} is shared with this project's other members and sets ${key}, which a chat on a key can't use.`,
  /** W120: Row, the Chat log, `blockSourceSaid` for `KeyProxy`. */
  W120: "Blocked by purlis's key proxy",
  /** W121: Row, the Chat log, `whoBlocked` for `KeyProxy`. */
  W121: "purlis's key proxy",
  /** W122: Row, the Chat log, `ALLOWED_SAID` `model_key`. */
  W122: "a model key",
  /** W123: Ask, Your keys (*Stop sending*'s confirmation). */
  W123: COUNTED_W123,
  /** W124: Label, New chat's footer (a button). */
  W124: ({ name }: { name: string }) => `Send key to pool ${name}`,
  /** W125: Label, Your keys (a button). */
  W125: "Stop sending",
  /** W126: Label, Needs you, Your keys (a button). */
  W126: "Mark again",
  /** W127: Label, the key forms (a button). */
  W127: "Save anyway",
  /** W128: Row, Your keys, under the key's row. */
  W128: ({ error }: { error: string }) => `Couldn't mark this key again: ${error}.`,
  /** W129: Row, Your keys, under the pool's line. */
  W129: ({ pool, error }: { pool: string; error: string }) =>
    `Couldn't stop sending this key to ${pool}: ${error}.`,
  /** W130: Refusal, New chat. */
  W130: ({ file, pool, key }: { file: string; pool: string; key: string }) =>
    `${file} on ${pool} sets \`models.${key}\`, which this version of purlis can't enforce. Chats can't start there.`,
} as const;

/** The systems the window runs on, as `std::env::consts::OS` names them. */
export type Os = "macos" | "linux" | "windows";

/**
 * **A line as this system says it**, the table's first two rules: on macOS *computer* reads
 * *Mac*, and off macOS *your keychain* reads *your system keychain*.
 */
export function onThisOs(line: string, os: Os): string {
  return os === "macos"
    ? line.replace(/computer/g, "Mac")
    : line.replace(/our keychain/g, "our system keychain");
}

/**
 * **The thing a runner chat's line names, on its pool**, the table's third rule: *<lock> on gpu*
 * in a runner chat's Notice or refusal, and the thing alone off a runner (`pool` is `null`).
 */
export function onPool(thing: string, pool: string | null): string {
  return pool === null ? thing : `${thing} on ${pool}`;
}
