# Window copy, phase 1a

The Window copy table of phase 1a's spec ("Any model on any harness, phase 1a: the tracer",
epic #6743), as the operator approved it. Every line lives in the copy module keyed by its
W-number: `windowCopy.ts` for the window and `crates/purlis-core/src/window_copy.rs` for core.
`windowCopy.test.ts` holds this table and both halves to each other, so a change to a line is a
change to this table and the module together, and a change to a line's words comes back to the
spec's table and the operator's yes first.
**Kind** is one of: *Ask* (waits for an answer, in Needs you), *Notice* (says what happened, with a way out), *Update* (a change the person made, counted), *Row* (a line on a card, row or field), *Refusal* (a sentence at a turn's end or a start), *Label* (a field's label, an option or a name). **Ways out** lists every button the line carries; "—" is none. Three rules hold for every row: the module swaps *computer* for *Mac* on macOS; "your keychain" reads *your system keychain* off macOS; a runner chat's Notice or refusal names its pool, *on <pool>*, after the thing it names (E-10, E-17, A-5). `counted` is the window's count helper (one, many and none forms).

| # | String | Kind | Place | Ways out | Source | Ticket |
|---|---|---|---|---|---|---|
| W1 | *"Paste your Anthropic key"* | Label | J0's form, the key form | — | design J0 | T1a-13 |
| W2 | *"Use the key you already have"* (the one wording in 1a; the indefinite form returns in 1b) | Label | J0's form, the key form, *Copy to your keys* | — | design J0, V7 | T1a-13 |
| W3 | `claude-anthropic` | Label | J0's form (pre-filled name) | — | U10 as V18 amends it, and X4 (the first free of `claude-anthropic`, `claude-anthropic-2`) | T1a-13 |
| W4 | *"No keys yet"* | Row | Your keys (empty state) | *Add a key…* | design §3.20 | T1a-11 |
| W5 | through `counted`: *"2 chats use this key. Their next request is blocked, and the conversation is kept."* · *"1 chat uses this key. Its next request is blocked, and the conversation is kept."* · *"No chats use this key."* | Ask | Your keys (Remove's confirmation) | *Remove*, *Cancel* | R-550, design §3.20 as V23 amends it | T1a-11, T1a-27 |
| W6 | *"This key can't be read. Unlock your keychain to use it."* | Row | Your keys | *Unlock* | drafted | T1a-11 |
| W7 | *"Not checked yet. purlis couldn't check this key with Anthropic."* | Row | J0's form, the key form, *Replace*, Your keys | *Try again* | drafted (U11, E-13) | T1a-38 |
| W8 | *"Anthropic didn't accept this key. Check it, or save it anyway."* | Row | J0's form, the key form, *Replace* | *Save anyway* | drafted (E-13) | T1a-38 |
| W9 | *"You already have an Anthropic key. Replace it?"* | Row | J0's form, the key form | *Replace the key…*, *Use the key you already have* | drafted (U9, V7) | T1a-13 |
| W10 | *"Your keychain is locked. Unlock it to start this chat."* | Ask | New chat; Needs you | *Unlock* | drafted | T1a-61 |
| W11 | *"purlis: your Anthropic key was removed. Add it again, then send your prompt again."* | Refusal | the turn's end | — | design §3.20 | T1a-27 |
| W12 | through `counted`: *"Your Anthropic key was removed. Add it again to use these chats: <chats>. The conversation is kept. Don't use `/login` here."* (one-chat form: *"… to use <chat>."*) | Ask | Needs you | *Add a key…*, *Start a new chat* | drafted (U8, V24) | T1a-27 |
| W13 | *"Your Anthropic key was changed outside purlis. Mark it again to use it. The conversation is kept. Don't use `/login` here."* | Ask | Needs you | *Mark again*, *Start a new chat* | drafted (U8, V24) | T1a-27 |
| W14 | *"Key missing"* | Row | New chat's card | *Add a key…* | R-487 | T1a-27 |
| W15 | *"Anthropic refused your key. Replace it, or start a new chat. Don't use `/login` here."* (on `authentication_error`) | Notice | Inbox | *Replace the key…*, *Start a new chat* | U21, V25 | T1a-30 |
| W16 | *"Anthropic is limiting requests on your Anthropic account. Try again after <time>."* (from `retry-after`, shown as the clock time it may be tried again, outside a live region) · without a `retry-after` or W87's mark: *"Anthropic is limiting requests on your Anthropic account. Try again later."* | Notice | Inbox | *Try again*, *Start a new chat*, Dismiss | drafted (E-10) | T1a-30, T1a-71 |
| W17 | *"Anthropic isn't answering. Try again, or start a new chat."* | Notice | Inbox | *Try again*, *Start a new chat* | drafted | T1a-30, T1a-71 |
| W18 | *"This conversation is too long for the model. Compact it in Claude Code, or start a new chat."* | Notice | Inbox | *Start a new chat* | drafted (R-521) | T1a-30 |
| W19 | *"Anthropic answered <status>: <message>. Start a new chat."*; for a 403 (`permission_error`): *"Anthropic says this key can't do that: <message>."* | Notice | Inbox | *Start a new chat* | drafted (R-541's 1a form, V1, V25) | T1a-30 |
| W20 | *"purlis can't reach Anthropic through this network. purlis uses this computer's proxy settings and trusted certificates."*; on a runner: *"Pool <name> can't reach Anthropic through its network."* | Notice | Inbox | *Try again*, *Start a new chat* | drafted (R-517, J-9, F-9) | T1a-30 |
| W21 | *"Anthropic's certificate isn't trusted on this computer. purlis uses this computer's proxy settings and trusted certificates."* (*on this Mac* on macOS); on a runner: *"… isn't trusted on pool <name>."* | Notice | Inbox | *Start a new chat* | drafted (R-517, E-11, J-9, F-9) | T1a-30 |
| W22 | *"This chat runs on your Anthropic key. Signing in to a Claude plan here does nothing."* | Notice | the pane's band (once per run) | *Resume* | design §3.20 | T1a-28 |
| W23 | *"Claude Code <version> didn't use the route purlis set"* | Notice | the pane's band | *End chat* | design §3.19 | T1a-42 |
| W24 | *"This project's Claude Code settings name another route. This chat uses purlis's."* | Row | card, chat details (said once) | — | design §3.20 | T1a-16 |
| W25 | *"purlis blocked this request: <reason>."* (`<reason>` from W80) | Refusal | the turn's end | — | drafted (R-117), V23 | T1a-26 |
| W26 | *"purlis: <lock> now blocks this."* | Refusal | the turn's end | — | U14, V23, E-7 | T1a-36 |
| W26a | *"<lock> now blocks this chat's requests."* | Notice | Inbox | *Start a new chat* | drafted (E-7) | T1a-36 |
| W26b | *"<lock> blocks <choice>. Nothing else can start this chat until the policy allows it."* | Refusal | New chat, the start | — | U14, E-7 | T1a-36 |
| W27 | *"Only an Anthropic key works in this version"* | Refusal | the card, the start | — | design §5.2 | T1a-65 |
| W28 | *"This profile's Anthropic key is in the vault <name>. Copy it to your keys to use it in any project?"* | Ask | card, Needs you | *Copy to your keys*, *Paste it again* | design §3.20 | T1a-39 |
| W29 | *"Set by your administrator in <file>: <key>, which a chat on a key can't use."* | Refusal | New chat | the card on Claude Code's own sign-in | R-245's words, the rest drafted (E-5) | T1a-41 |
| W29a | *"<file> sets <key>, which a chat on a key can't use."* | Refusal | New chat | *Open the file* | drafted (R-235, E-5) | T1a-41 |
| W30 | *"Your Anthropic key will be sent to pool <name> for its chats."* | Ask | above the footer; Needs you (the pool key allowance) | *Send key to pool <name>* | design §3.20, V20 | T1a-53 |
| W31 | *"Its admins can read a key held there for a run."* | Row | the *Runs on* row (said once) | — | drafted (U24) | T1a-53 |
| W32 | *"purlis sends a key only to a runner marked as yours. <pool> isn't marked."* | Row | New chat (unavailable card) | the pool's switch | design §3.20 | T1a-52 |
| W33 | *"Pool <name> runs an older purlis, so a key can't be sent there yet."* | Row | the *Runs on* row | *Update pool <name>…* | design §3.20 | T1a-54 |
| W34 | *"The link to pool <name> dropped. Its chat keeps your key until it ends."* | Notice | Inbox | *Start a new chat* | drafted (R-503, E-14) | T1a-56 |
| W35 | *"Pool <name>'s host restarted. Its chat ended."* | Notice | Inbox | *Resume* | drafted (R-504, E-14) | T1a-56 |
| W36 | *"<file> requires purlis <version> or later (`min-version`). Chats can't start. Update purlis."* | Refusal | New chat | — | drafted (U15, E-14, H-17) | T1a-35 |
| W37 | *"Metered"*, *"Reported by Claude Code"*, *"Not reported"*, *"Price unknown"*, *"Estimated"*, *"No calls yet"*, *"Spend not fully reported"* | Row | harness card tab (spend words) | — | design §3.13, V4 | T1a-34 |
| W38 | *"This chat has 64 model calls running at once. Wait for them to finish, then send your prompt again."* (only if the turn ends on the cap; cleared by that chat's next successful call) | Notice | Inbox | Dismiss (the one failure Notice without *Start a new chat*) | drafted (J-9) | T1a-30 |
| W39 | *"purlis can't use a key approval setting other than off in this version: <setting>."* | Refusal | New chat | — | drafted (R-663) | T1a-65 |
| W40 | *"purlis can't use a key from <1Password \| HashiCorp Vault> in this version."* | Refusal | New chat | — | drafted (R-663; names a Vault as a key's source, so R-020's lint exempts it as it does W28) | T1a-65 |
| W41 | *"purlis can't use a cloud sign-in in this version: <cloud>."* | Refusal | New chat | — | drafted (R-663) | T1a-65, T1a-39 |
| W42 | *"purlis can't use a local model server in this version: <server>."* | Refusal | New chat | — | drafted (R-663) | T1a-65 |
| W43 | *"purlis can't use a project's model provider file in this version: <file>."* | Refusal | New chat | *Open the file* | drafted (R-663) | T1a-65 |
| W44 | *"purlis can't use a chosen model in this version: <file>."* | Refusal | New chat | *Open the file* | drafted (R-663, V16) | T1a-65 |
| W45 | *"Send your prompt again."* | Row | Inbox, under W20, W16, W17, W38; after *Try again* or an answered Ask it is an Update in the same place until the next call or a dismiss (a Notice line) | Dismiss | design §3.19 (V1, V24) | T1a-30, T1a-71, T1a-27 |
| W46 | *"Anthropic didn't accept this key when you saved it."* (`refused at save`) | Row | Your keys | *Replace the key…* | drafted (V10) | T1a-38 |
| W47 | *"Pool <name> · Needs an update"* | Row | the *Runs on* row | *Update pool <name>…* | design §3.18, J9 step 3 | T1a-54 |
| W48 | *"Running chats keep going on the old runner"* | Row | under W47 | — | design §3.18, J9 step 3 | T1a-54 |
| W49 | *"Updating…"* | Row | the *Runs on* row (busy) | — | design §3.18 | T1a-54 |
| W50 | *"<pool> can't be reached. The chat there keeps this key until it ends or the link returns. To stop it now, revoke the key at Anthropic."* | Row | Your keys (drawn with W5's confirmation) | Dismiss | design §3.19 (R-477) | T1a-56 |
| W51 | *"Locked by policy, set by <who> in <file>."* | Row | the card | — | design §3.20 | T1a-36 |
| W52 | *"Locked by policy on pool <name>, set by <who> in <file>."* | Row | the card | — | drafted (T1a-67's form) | T1a-67 |
| W53 | *"routed by your administrator's Claude Code settings"* | Row | the card, chat details | — | design §3.7 (R-245) | T1a-41 |
| W54 | *"purlis can't read <source>, so it wasn't checked."* | Row | chat details | — | drafted (R-244) | T1a-41 |
| W55 | *"This key's entry is missing from your keychain."* | Row | Your keys | *Remove* | drafted (E-13, an orphaned index line) | T1a-11 |
| W56 | *"Add your key again in Settings › You › Profiles"* | Refusal | the card, the start | — | R-651 | T1a-39 |
| W57 | *"This is a Claude plan sign-in, not an API key, so purlis can't copy it to your keys."* | Row | under W28 | *Paste it again* | drafted (V7) | T1a-39 |
| W58 | *"as of <time>"* | Row | harness card tab (spend line suffix) | — | design §3.13 | T1a-34, T1a-56 |
| W59 | *"purlis: your Anthropic key was changed outside purlis. Mark it again, then send your prompt again."* | Refusal | the turn's end | — | drafted (E-6, W11's sibling) | T1a-27 |
| W60 | *"Key changed outside purlis"* | Row | New chat's card, Your keys' row | *Mark again* | drafted (E-6, F-3, J-4, E-3) | T1a-27, T1a-11 |
| W61 | *"purlis reopened this chat's connection to Anthropic."* | Update | the pane's band | Dismiss (R-536: a successful re-bind carries no other action) | drafted | T1a-63 |
| W62 | *"purlis couldn't reopen this chat's connection to Anthropic, so its run ended."* | Notice | the pane's band | *Resume* | drafted (R-536) | T1a-63 |
| W63 | *"purlis can't start this chat: no local port could be opened for it."* | Refusal | New chat | — | drafted (T1a-20, T1a-63) | T1a-20, T1a-63 |
| W64 | *"<file> can't be read: <error>. Chats on a key can't start until it's fixed."* | Refusal | New chat | *Open the file* | drafted (R-128, H-17, E-9) | T1a-35 |
| W65 | *"purlis: <file> can't be read (<error>), so this request was blocked."* | Refusal | the turn's end | — | drafted (R-128, H-17) | T1a-70 |
| W66 | *"<file> sets `models.<key>`, which this version of purlis can't enforce. Chats can't start."* | Refusal | New chat | *Open the file* | drafted (R-430, H-17) | T1a-35 |
| W67 | *"purlis: this chat waited 15 minutes for your computer, so the request was blocked. Open purlis on your computer, then send your prompt again."* | Refusal | the turn's end | — | drafted (design §3.19) | T1a-56 |
| W68 | *"Waiting for your computer to link again before this chat can restart."* | Row | the chat's card | — | drafted (R-468) | T1a-56 |
| W69 | *"<card> can't run on pool <name>. Pick another card."* | Row | New chat | — | drafted (R-475) | T1a-56 |
| W70 | *"This chat can't move to pool <name>: its key route can't run there. <what would work>."* | Refusal | the move | — | drafted (R-476) | T1a-52 |
| W71 | *"purlis doesn't carry a Claude plan from this computer to pool <name>. Use a profile signed in on the runner."* | Row | New chat (unavailable card) | — | drafted (U13) | T1a-52 |
| W72 | *"Each vault value or token still asks you when it is sent. A model key is sent once you allow it for this pool."* | Row | the pool's switch | — | drafted (U24, E-9: `MY_OWN_SAID` amended) | T1a-53 |
| W73 | *"Pool <name> couldn't update: <error>."* | Row | under W47 | *Try again* | drafted | T1a-54 |
| W74 | *"Pool <name> runs your team's image. Update the image to send keys there."* | Row | the *Runs on* row | — | drafted | T1a-54 |
| W75 | *"Pool <name> runs purlis <runner version>, which doesn't match this purlis (<version>). Update the pool to send keys there."* | Row | the *Runs on* row | *Update pool <name>…* | drafted (R-471) | T1a-51 |
| W76 | *"For <chat>"* | Row | the harness card tab's spend block | — | drafted (V19) | T1a-34 |
| W77 | *"purlis last tested Claude Code <version>."* | Row | under W23 | — | drafted | T1a-42 |
| W78 | *"purlis: this key is for Anthropic and can't be sent to <origin>."* · at a start, without the prefix: *"This key is for Anthropic and can't be sent to <origin>."* | Refusal | the turn's end; a refused start | — | drafted (R-179) | T1a-17 |
| W79 | *"Calls to model providers go through purlis's key proxy and show here as this chat's."* | Row | the Network page | — | drafted (R-411) | T1a-19 |
| W80 | W25's `<reason>`, a closed set: *"it carried no run token"*, *"<method> <path> isn't a model call"*, *"its model couldn't be read"*, *"it repeated a field"*, *"its body was encoded"*, *"its body couldn't be read whole"*, *"it carried a Claude plan sign-in"*, *"it came from a web page"*, *"its body is over 128 MB"*, *"it carried a plan sign-in"* | Refusal | the turn's end (part) | — | drafted (J-2) | T1a-26, T1a-20, T1a-23, T1a-28 |
| W81 | A 502 or 504's `purlis:` cause, a closed set: *"purlis couldn't find <host>."*, *"purlis couldn't connect to <host>."*, *"<host>'s certificate isn't trusted."*, *"The outbound proxy refused the connection."*, *"<host> didn't answer within <n> seconds."*, *"<host> reset the connection."*, *"purlis can't use the outbound proxy's <scheme> scheme."* | Refusal | printed by Claude Code when it gives up (transport) | — | drafted (J-2) | T1a-26, T1a-21 |
| W82 | *"purlis: this chat has 64 model calls running at once."* | Refusal | printed by Claude Code when it gives up (the cap's 429) | — | drafted (J-2) | T1a-22 |
| W83 | *"purlis: waiting for room to hold this request."* | Refusal | printed by Claude Code when it gives up (the body-wait 429) | — | drafted (V6) | T1a-22 |
| W84 | *"purlis couldn't unlock your keychain."* | Row | under W10 | *Unlock* | drafted (J-2, `UnlockAskFailed`) | T1a-61 |
| W85 | *"Your Anthropic account has reached its spend limit: <message>. Raise it at Anthropic, then send your prompt again."* (a 400 whose message starts with one of the two documented spend-limit prefixes) | Notice | Inbox | *Start a new chat* | drafted (C-8) | T1a-30 |
| W86 | *"Anthropic reports a billing problem on your Anthropic account: <message>. Fix it at Anthropic, then send your prompt again."* (a 402 `billing_error`) | Notice | Inbox | *Start a new chat* | drafted (C-8) | T1a-30 |
| W87 | *"Anthropic has paused requests on your Anthropic account until its usage limit resets: <message>."* (a 429 whose `error.details.error_code` is `enforced_spend_limit_reached`) | Notice | Inbox | *Start a new chat* | drafted (C-8) | T1a-30 |
| W88 | *"purlis couldn't replace this key: <error>."* | Row | Your keys, the key form | *Try again* | drafted (G-7) | T1a-11 |
| W89 | *"purlis couldn't save this key: <error>."* | Row | J0's form, the key form | *Try again* | drafted (G-7) | T1a-13 |
| W90 | *"Its key is missing from your keys"* | Row | You › Profiles | *Add a key…* | drafted (G-7) | T1a-13 |
| W91 | *"<lock> doesn't let purlis reach Anthropic to check this key."* | Row | J0's form, the key form | *Save anyway* | drafted (G-7) | T1a-38 |
| W92 | *"A local harness declaration can't set `route.<field>` in this version."* | Refusal | the start | — | drafted (R-224) | T1a-37 |
| W93 | *"This chat runs on your Anthropic key. Signing in to another plan here does nothing."* | Notice | the pane's band (once per run) | *Resume* | drafted (E-13, W22's sibling for a non-Claude shape) | T1a-28 |
| W94 | *"about a model key"* (completes *"Asked you …"*) | Row | the Chat log, `ASK_SAID` `model_key` | — | drafted (round-2 Batch A, V25), after main's `ASK_SAID` | T1a-61 |
| W95 | *"about sending a key to a pool"* (completes *"Asked you …"*) | Row | the Chat log, `ASK_SAID` `pool_allow` | — | drafted (round-2 Batch A, V25) | T1a-61 |
| W96 | *"Model key"* / *"Model keys"* | Label | the Notices view, `NOTICE_KINDS` (Notice kind, one / many) | — | drafted (round-2 Batch B), after main's `noticeKinds.ts` | T1a-30 |
| W97 | *"Model call cap"* / *"Model call caps"* | Label | the Notices view, `NOTICE_KINDS` (Notice kind, one / many) | — | drafted (round-2 Batch B) | T1a-30 |
| W98 | *"Port <port> is purlis's own key proxy, so no chat can be granted it."* | Refusal | the port grant form's error state | — | drafted (R-068, round-2 Batch B) | T1a-20 |
| W99 | *"blocked by purlis's key proxy"* | Row | the Blocked log, `network.rs`'s words for `Source::KeyProxy` | — | drafted (V17), after main's `Source` lines | T1a-26 |
| W100 | *"The lock is <lock>."* | Row | after W51, on a refused card or row (a second sentence) | — | drafted (E-14) | T1a-36 |
| W101 | *"`charter.local.toml` sets `[sign_in] plans` for <harness>, which only purlis's own declaration may set. purlis uses its own."* | Refusal | where refused declarations are shown (`Declarations::refused`) (reason) | — | drafted in T1a-37 (R-224, H-13) | T1a-37 |
| W102 | *"Signs in with"* | Label | Add profile's form | — | design J0 (X4) | T1a-13 |
| W103 | *"Its own sign-in"* (the default option) | Label | Add profile's form | — | drafted (X4) | T1a-13 |
| W104 | *"A key"* (an option) | Label | Add profile's form | — | drafted (X4) | T1a-13 |
| W105 | *"Only Claude Code can use a key in this version."* | Refusal | Add profile's form, under *A key* shown off for another harness; the card of an old non-Claude-Code profile holding an Anthropic key | the old profile: *Copy to your keys* | drafted (X4, X10) | T1a-13, T1a-39 |
| W106 | *"<file> on <pool> requires purlis <version> or later (`min-version`). Chats can't start there."* (W36's runner form; every chat that reaches a model, subscription included, X6) | Refusal | New chat | *Update pool <name>…* | drafted (X6) | T1a-77 |
| W107 | *"<file> on <pool> can't be read: <error>. Chats on a key can't start there until it's fixed."* (W64's runner form) | Refusal | New chat | *Update pool <name>…* | drafted (X6) | T1a-77 |
| W108 | *"purlis: <file> on <pool> can't be read (<error>), so this request was blocked."* (W65's runner form) | Refusal | the turn's end | — | drafted (X6) | T1a-77 |
| W109 | *"purlis: <file> requires purlis <version> or later, so this request was blocked."* (a raised `min-version` mid-run; on a runner the pool rule above adds *on <pool>*) | Refusal | the turn's end | — | drafted (X6, J-5) | T1a-70, T1a-77 |
| W110 | *"purlis: <lock> on <pool> now blocks this."* (a runner lock at the turn's end) | Refusal | the turn's end | — | drafted (X6, A-5) | T1a-67 |
| W111 | *"Anthropic refused the key this chat held. purlis read your key again. Send your prompt again."* (the cured 401; cleared by that chat's next call) | Notice | Inbox | Dismiss | drafted (V1, E-4, J-9) | T1a-30 |
| W112 | *"purlis couldn't save sending your Anthropic key to <pool>: <error>. The chat didn't start."* (`PoolAllowStoreFailed`) | Refusal | New chat's footer | *Send key to pool <name>* | drafted (E-5) | T1a-53 |
| W113 | *"Sent your Anthropic key to <pool> and started the chat."* (the allowance's form of the turned-Start sentence, `turnedSaid`) | Update | New chat's footer | — | drafted (E-5) | T1a-53 |
| W114 | *"purlis couldn't remove this key: <error>. It is still in your keys."* | Row | Your keys | *Remove* | drafted (E-5, G-7) | T1a-11 |
| W115 | *"Your keys can't be read: <error>."* | Row | Your keys | *Try again* | drafted (G-7) | T1a-11 |
| W116 | *"<pool> can't be reached right now."* (main's existing link-down string is used instead where one exists) | Row | New chat's card | — | drafted (F-12) | T1a-56 |
| W117 | *"Chats on a key start in a later update of this build."* (a key profile, until T1a-17 lands) | Refusal | New chat | — | drafted (E-5) | T1a-12 |
| W118 | *"This profile's key is stored the old way. A later update of this build copies it to your keys."* (an old record, until T1a-39 lands) | Refusal | New chat | — | drafted (E-5) | T1a-12 |
| W119 | *"<file> is shared with this project's other members and sets <key>, which a chat on a key can't use."* (W29a's committed-file form) | Refusal | New chat | *Open the file* | R-245's words, the rest drafted (E-5) | T1a-41 |
| W120 | *"Blocked by purlis's key proxy"* | Row | the Chat log, `blockSourceSaid` for `KeyProxy` | — | drafted (E-3, V17) | T1a-26 |
| W121 | *"purlis's key proxy"* | Row | the Chat log, `whoBlocked` for `KeyProxy` | — | drafted (E-3) | T1a-26 |
| W122 | *"a model key"* (completes main's `ALLOWED_SAID` sentence) | Row | the Chat log, `ALLOWED_SAID` `model_key` | — | drafted (E-17) | T1a-61 |
| W123 | through `counted`: *"Stop sending this key to <pool>? 2 chats there stop at their next request, and their conversations are kept."* · *"Stop sending this key to <pool>? 1 chat there stops at its next request, and its conversation is kept."* · *"Stop sending this key to <pool>? No chats there use it."* | Ask | Your keys (*Stop sending*'s confirmation) | *Stop sending*, *Cancel* | drafted (X1, F-9) | T1a-11, T1a-74 |
| W124 | *"Send key to pool <name>"* | Label | New chat's footer (a button) | — | design §3.18 (U22) | T1a-53 |
| W125 | *"Stop sending"* | Label | Your keys (a button) | — | design §3.20 (V8) | T1a-11 |
| W126 | *"Mark again"* | Label | Needs you, Your keys (a button) | — | design §3.20 (U8) | T1a-27 |
| W127 | *"Save anyway"* | Label | the key forms (a button) | — | design §3.5 (U11) | T1a-38 |
| W128 | *"Couldn't mark this key again: <error>."* | Row | Your keys, under the key's row | *Mark again* | drafted (U8, E-3) | T1a-11 |
| W129 | *"Couldn't stop sending this key to <pool>: <error>."* | Row | Your keys, under the pool's line | *Stop sending* | drafted (X1) | T1a-11 |
| W130 | *"<file> on <pool> sets `models.<key>`, which this version of purlis can't enforce. Chats can't start there."* (W66's runner form; every chat that reaches a model, subscription included) | Refusal | New chat | *Update pool <name>…* | drafted (X6, R-430, final-integrity 3) | T1a-77 |
| W131 | *"purlis: <file> on <pool> sets `models.<key>`, which this version of purlis can't enforce, so this request was blocked."* (a `models` key arriving mid-run on a runner) | Refusal | the turn's end | — | drafted (X6, R-430, final-integrity 3) | T1a-77 |
| W132 | *"<lead>, and on <system> purlis can't keep <harness> chats away from the system keyring, where your model keys are, so nothing was started. Start this chat without the sandbox from the new-chat picker, or remove your model keys in Your keys."* · where the chat may not start without the sandbox (resumed, relaunched, or policy requires it): *"<lead>, and on <system> purlis can't keep <harness> chats away from the system keyring, where your model keys are, so nothing was started. Remove your model keys in Your keys to start it sandboxed."* | Refusal | New chat; a sandboxed chat's start (`NotStarted`, the credential service unheld because of a model key) | the new-chat picker's *Start without the sandbox* | drafted in T1a-72 (D6640-T3 item 7; the security review of #6764) | T1a-72 |
| W141 | *"There is no undo."* (after W5) | Row | Your keys (Remove's confirmation) | — | drafted (U19), in T1a-11's fix round | T1a-11 |
| W142 | *"Remove the key for <product>?"* | Ask | Your keys (Remove's confirmation, its title) | *Remove*, *Cancel* | drafted (U19), in T1a-11's fix round | T1a-11 |
| W143 | *"purlis couldn't check this key: <error>."* | Row | Your keys, under the key's row | *Try again* | drafted (F-6), in T1a-11's fix round | T1a-11 |
| W144 | *"Asks before each use: <setting>"* | Row | Your keys | — | drafted (R-171), in T1a-11's fix round | T1a-11 |

R-020's lint exempts W28 and W40, which name a Vault as a key's source.
