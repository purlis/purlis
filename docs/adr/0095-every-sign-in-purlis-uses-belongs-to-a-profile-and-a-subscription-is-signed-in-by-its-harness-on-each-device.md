# Every sign-in purlis uses belongs to a profile, and a subscription is signed in by its harness on each device

**Accepted 2026-10-14** by the operator, in the grill rounds of 2026-10-14 on the profiles spec
(#2030), ticket #2032. It replaces the promise of [ADR 0087](0087-a-chats-model-is-one-choice-from-four-sources-and-charter-never-carries-a-harness-login.md)
§5, carried from V9, that *"purlis never reads, copies, stores or relays a harness credential"*,
with the rule below, and amends ADR 0087, ADR 0078 and ADR 0080 in sections of their own at the
end. Three readings of ADR 0087 are the dispatcher's, to be confirmed (*Decided (dispatcher), to be
confirmed*, at the end). It follows these of the operator's rulings of 2026-10-14:

- **Q27 and Q30 to Q35** (rounds 3 and 4, the operator's own idea), in short: a profile becomes
  first-class. purlis owns each profile's folder, imports its settings from the machine's harness
  and never its credentials, keeps its conversations in it, and pins its harness version. A
  profile is chosen like a model choice, is the person's, and is never committed. API keys and
  cloud credentials are vault values sent to devices the person owns. Any subscription is signed
  in once per device through the harness's own flow, shown in the window, and purlis never reads,
  copies, stores or relays it. A Claude plan is never offered in opencode. This record replaces
  ADR 0087's and V9's promise.
- **No loss on migration**, the operator's condition: *"users should not lose their profiles and
  sessions."* The machine's own harness folder is adopted in place as a profile, so nothing is
  signed in again and nothing is lost.
- **The simplification round** (Q36 to Q43), in part: a **profile** holds a harness's sign-in and
  settings and the browser logins kept under it. The person owns it, and a persona names one.
- **The sandbox round** (Q46 to Q54), in part: a profile's credential files are denied to the
  agent's commands wherever the harness runs its commands apart from itself, and otherwise the
  profile page says that commands can read them. A sandboxed Codex chat uses its profile's folder.
- **X12**, through ADR 0087 §3: *"unattended runs (triggers, gardeners, workflows, off-peak,
  headless, race) prefer API-key or local-model sources, with a one-time acknowledgement before a
  consumer subscription runs unattended"*. It stays.

It builds on ADR 0067 (the sandbox), ADR 0068 (one path to a vault value), ADR 0072 (the five
concepts), [ADR 0073](0073-a-harness-is-declared-as-data-and-a-chat-runs-it-at-one-of-three-levels.md)
(harness declarations, which say the sign-in kinds each harness supports, #2031) and ADR 0087 (the
model choice and a key's path). Its concept is **Chat**: a chat runs on one profile, beside its
harness and persona.

## The decision

**purlis never touches the person's machine sign-in of a harness, except by adopting its folder
in place as a profile. Every sign-in purlis uses belongs to a profile. A subscription is signed in
through the harness's own flow, once per device, shown in the window, and purlis never reads,
copies, stores or relays it. An API key or cloud credentials are vault values, sent only to
devices the person owns.**

### 1. A profile holds the sign-in

- **A profile is one identity of a harness on a device**: the harness, its settings, its sign-in,
  its conversations and the browser logins kept under it, in a folder purlis owns. purlis sets
  the harness's own folder variables to that folder (ADR 0073, as amended by #2031), so the
  harness keeps its sign-in there.
- **Nothing purlis starts uses the machine's default harness folder** unless that folder was
  adopted as a profile (§2).
- **A profile is the person's.** It lives on its device and is never committed to the plane or a
  clone. A profile made in the window is available in every project on this machine; one declared
  in a clone's local file works as it is declared. A persona names the profile it prefers, and a
  teammate without one of that name is offered to make their own.
- **A profile never names a credential in its environment.** ADR 0073 §5's refusal stands. A key
  reaches a chat only by the adapter, from a vault reference (§4).

### 2. The machine's sign-in is adopted, never touched

- **Each harness folder the machine already uses is adopted in place** as a profile named after
  the harness (*"Claude Code (this Mac)"*): the same folder, the same sign-in, the same settings
  and the same conversations. Nothing is copied or moved, and no one signs in again. Profiles the
  person makes afterwards get folders purlis owns.
- **A chat running when the adoption happens keeps its run**, and its next run uses the adopted
  profile. Every chat resumes on its own conversation.
- **A new profile's settings are imported from the machine's harness settings, never from its
  credential files.** The profile records when, and offers re-import. From then on the profile's
  settings are what purlis chats use.

### 3. A subscription is the harness's, on each device

A subscription (a Claude plan, ChatGPT, Copilot and their like) is the harness's sign-in kind
`subscription` (ADR 0073, as amended by #2031).

- **It is signed in through the harness's own flow**, run in the profile's folder and shown in the
  window as an **Ask** of the person: the harness's own browser page or device code. The person
  never types a password into purlis.
- **The harness keeps the result in the profile's folder.** purlis never reads, copies, stores or
  relays it, and never points it anywhere but its vendor (ADR 0087 §5's routing rules stand).
- **One sign-in per device.** Each device a profile runs on signs it in there, a runner included,
  and the sign-in never travels between devices. Each device's harness then refreshes its own
  sign-in alone.
- **On a runner, a subscription profile needs a kept claim** (Q30 to Q35); how is spec #2041's.
- **A subscription a harness may not carry is never offered.** A Claude plan is never offered in
  opencode. Anthropic's terms forbid a third party from storing or intermediating Claude
  subscription tokens and require Anthropic's own sign-in flow, and this rule keeps to them.
- **A profile that is not signed in says so before a chat starts**, with Sign in as the way out.
  A chat never starts half signed in.

### 4. Keys and cloud credentials are vault values

- **An API key** (`api-key`) **and cloud credentials** (`cloud`: Bedrock, Vertex, Azure) live in
  the person's vault. The profile names them by vault reference, and the adapter hands them to the
  chat at start, through ADR 0087 §7's path: the proxy where the harness takes a base URL, its
  process environment otherwise, under the key's approval setting (V15).
- **Ruled 2026-10-15 (#2151):** cloud credentials go into the harness's process environment,
  since a proxy that swaps one header cannot carry a cloud's signed requests; #2151 opens that
  path. A key keeps waiting for the key proxy (#822). ADR 0087's *A model choice is a profile
  plus a model* records how.
- **They are sent only to devices the person owns.** A runner not marked owned never gets one.
- **A local model** (`local`) has nothing to sign in.

### 5. A profile's credentials are kept from the agent's commands

- **A profile's credential files are a class the sandbox always denies to the agent's commands**
  wherever the harness runs its commands apart from itself (ADR 0067). A sandboxed chat, Codex's
  included, uses its profile's folder directly, and no sign-in is copied anywhere.
- **Where a harness cannot keep its commands apart from itself**, its declaration says so
  (`kept_from_commands`, #2031) and the profile page says plainly that commands can read its
  sign-in.
- *Amended 2026-10-15 (#2037), by the operator's ruling:* a sandboxed Codex chat's profile is
  one purlis made **for sandboxed chats only**, signed in once through Codex's own sign-in in its
  folder and started by no chat outside the sandbox; a sandboxed Codex chat on any other profile
  is refused with the way out. So the person's own Codex home, adopted in place, is never written
  by a sandboxed chat (ADR 0067, D-88q, as amended).

### 6. Unattended runs still ask once

**Settled by X12**, which stands unchanged, as ADR 0087 §3 and §11 record it: an unattended run
prefers an API-key or local-model source, and purlis asks once, per project, harness and login on
this machine, before a subscription runs unattended. The run waits as a needs-you item until the
acknowledgement is given.

## ADR 0087, amended

- **§5's opening line is replaced.** It read *"Settled by V9: purlis never reads, copies, stores
  or relays a harness credential."* It now reads: **purlis never touches the person's machine
  sign-in except by adopting its folder in place. Every sign-in purlis uses belongs to a profile.
  A subscription is signed in through the harness's own flow per device, shown in the window, and
  purlis never reads, copies, stores or relays it. A key or cloud credentials are vault values
  sent to owned devices.** The rest of §5 stands: a `login` chat starts with no routing variables,
  committed harness config cannot reroute it, and smart model choice never moves it.
- **§5's "Logging in is the harness's own flow, in a shell tab"** becomes: signing in is the
  harness's own flow, run in the profile's folder and shown in the window as an Ask (§3 above).
- **§9's "`login` on a runner is the runner's own login, made in a shell tab on the runner"**
  becomes: a subscription profile is signed in on the runner through the harness's own flow,
  shown in the window, and the desktop's sign-in never travels.
- **§3's unattended acknowledgement** stands as it is (§6 above).
- How a model choice names a profile is recorded with #2036, in ADR 0087's *A model choice is a
  profile plus a model*.

## ADR 0078, amended

- **§9's "Harness logins stay in the harness's flow"** is read with §3 above: a profile is signed
  in on the runner through the harness's own device or paste-code flow, shown in the window. purlis
  never reads, copies, stores, vaults or relays a subscription's sign-in. A key or cloud
  credentials reach a runner only as ADR 0087 §9 forwards them, and only to a runner the person
  owns.

## ADR 0080, amended

- **§4's authentication bullet stands**: purlis never supplies a credential or any other value to
  `authenticate`. A profile's key reaches the harness at start (§4 above), never through
  `authenticate`. Where no usable method remains, purlis offers to sign the chat's profile in
  through the harness's own flow, shown in the window (§3 above), in place of a shell tab.

## What changes where

The code does not change with this record.

| Where | Change |
|---|---|
| `CONTEXT.md` | **Profile** and **Model source** cite this record; Profile's avoid list names the words it replaces (in this PR) |
| ADR 0087, ADR 0078, ADR 0080 | Amended above, each with a line pointing here |
| #2031 | Harness declarations say each harness's sign-in kinds, folder variables, `kept_from_commands` and how a conversation moves (ADR 0073, amended) |
| #2033 | Each existing harness folder adopted in place as a profile, losing nothing |
| #2034 | Making, importing and removing profiles. `crates/purlis-core/docs/control-plane.md` (*No credentials in a profile*) says a profile is signed in from the window or `purlis profile` |
| #2035 | A subscription signed in through the harness's own flow, as an Ask, and the same section of `control-plane.md` |
| #2036 | `api-key` and `cloud` profiles from the vault; a model choice is a profile plus a model |
| #2037 | A profile's credential files denied to the agent's commands. `SECURITY.md` (*What Codex keeps for itself*) says a sandboxed Codex chat keeps its state in its profile's folder |
| #2038 | A profile pins its harness version |
| #2039 | Personas, teammates, and the rules that stay: the permission-flag refusal, dispatch lists and approval; `[sign_in] plans` in harness declarations (ADR 0073, amended 2026-10-15) |
| #2168 | X12: the acknowledgement before a subscription runs unattended, and its store, `.charter/unattended-logins.json` in `docs/plane-format.md`, keyed as ADR 0087 keys it |

## What this costs

- **A sign-in per device.** A person with a desktop and two runners signs a subscription profile
  in three times, once on each.
- **Folders purlis owns.** A profile's settings and conversations live in its folder, not in the
  machine's default harness folder, unless that folder was adopted.

## What was rejected

- **Migrating the machine's sign-in into a new folder.** Copying a sign-in breaks harnesses whose
  refresh tokens are single use, and the operator's condition is that nothing is lost. Adoption in
  place keeps the folder as it is.
- **purlis holding a subscription's tokens to move them between devices.** Anthropic's terms
  forbid it, and refresh tokens that rotate would fight across devices. Each device signs in.
- **Keeping every chat on the machine's default harness folder.** It ties every chat to one account
  per harness and leaves nothing a profile can name.

## Decided (dispatcher), to be confirmed

Three readings of ADR 0087 were not ruled, and the dispatcher decided them on 2026-10-14, to be
confirmed by the operator:

- **The source `login` is a profile's `subscription` sign-in.** ADR 0087 §2's "whatever
  `claude /login`, `codex login` or the harness's own flow set up" is read as the sign-in the
  harness keeps in the chat's profile folder.
- **A key the harness itself stores in a profile's folder is still the harness's**, and purlis
  does not read it (ADR 0087 §2's last bullet, applied to profiles).
- **ADR 0087 §5's "the user's own global harness config"** is the adopted profile's own settings
  (§2 above), and a profile purlis made has the settings it imported. Each is the person's choice,
  as §5 says.

How a folder is adopted (§2, #2033) was not ruled beyond the operator's condition that no profile
or session is lost. The dispatcher decided it on 2026-10-14 in implementation, to be confirmed by
the operator:

- **An adopted profile keeps its built-in's name** (`claude`, `codex`, `opencode`) and is shown
  by its title, *Claude Code (this Mac)* (*this computer* off macOS). Every chat record, persona
  and dispatch list already names the built-in, so each chat's next run is on the adopted profile
  with nothing rewritten. A profile a clone's local file declares, under that name or another,
  is not adopted and runs as it is declared.
- **The folder adopted is the one a chat on the built-in reaches**: the one the harness's folder
  variable (`[folders]`, ADR 0073) names in the environment the app starts its chats from, and
  otherwise the harness's default (`~/.claude`, `~/.codex`, opencode's folder under `~/.config`
  and `~/.local/share`). It is adopted only where it is there and is the person's own: named by
  an absolute path, owned by this user, and outside every project and git work tree. The app
  adopts at its launch, before any chat starts; `purlis profile list` only shows what was adopted.
- **A chat on an adopted profile starts exactly as it did on the built-in**, so it reaches the
  same folder the same way, and purlis sets no folder variable for it. The record says which
  folder variables were set when the folder was found, so later profile work (#2034) reaches it
  by those and no others. A default folder is reached with none set: Claude Code given
  `CLAUDE_CONFIG_DIR`, even naming its default folder, looks for its sign-in elsewhere.
- **The record is in this machine's store**, `<config>/profiles.json` (`docs/plane-format.md`),
  where the profiles the window makes (#2034) are kept too. Never in the harness's folder and
  never in a committed file. A folder a record already points at, a built-in name a record
  already has, and a folder the person removed as a profile are not adopted again, so a second
  upgrade changes nothing.

How a subscription is signed in and how purlis knows (§3, #2035) was not ruled beyond §3 above.
The implementer decided it on 2026-10-15, to be confirmed by the operator:

- **A start refused because its profile is signed out raises one Ask**, "Sign in <profile>",
  in the Inbox, one per project and profile however many starts were refused, dispatched and
  unattended starts included; the picker's refusal points to it. It is about no chat, so it is
  listed under the profile. **The Ask's action is a shell tab**, so purlis never reads the flow:
  Sign in opens a shell tab at the project's root, and the core types the harness's own sign-in
  into it (`claude auth login`, `codex login`, `opencode auth login --provider <plan>`) with the
  profile's folder variables set; `purlis profile sign-in <name>` runs the same in the person's
  own terminal. Either way it runs as the person, outside every chat's sandbox, and purlis reads
  none of what it prints. The browser page or code the harness shows is seen in that tab, not
  read out of it. The Ask leaves the Inbox once the harness says the profile is signed in (it is
  asked again every few seconds after Sign in), or when the person dismisses it.
- **opencode is signed in to a plan purlis names**: the Ask and the profile page offer ChatGPT
  and GitHub Copilot, passed as opencode's own `--provider`, and never Anthropic, so a Claude
  plan is never signed in through opencode (#2039). Since #2039 the plans are opencode's declaration's
  `[sign_in] plans`, and `anthropic` is refused in any declaration but Claude Code's own (ADR
  0073, amended 2026-10-15).
- **What a harness says is remembered for a minute**, and forgotten by Sign in, so reopening many
  chats on one profile asks it once.
- **No sign-in is typed on Windows**, whose shells have no `env`; a word with a quote, a
  backslash or a control character in it is never typed into any shell.
- **Signed in is the harness's own answer, and purlis opens no file to get it**: how its status
  command exits (`claude auth status`, `codex login status`: 0 signed in, 1 signed out), with
  nothing it prints read; where a harness has none that says signed out, whether its credential
  file is there by its name (opencode's `auth.json`), and no file there is no answer. The words
  are a list in purlis's code (`profilestore::sign_in`), beside `sign_in_variables`, not yet
  declaration data.
- **Only a profile purlis made is asked, and only "signed out" refuses a chat.** An adopted
  folder is signed in by the person's own sign-in and never asked again, a built-in or a clone's
  profile runs as it always did, a profile handed a key or a cloud switch is not asked, and a
  harness that cannot say, or does not answer in time, never stops a chat that might work.

How a teammate is offered the profile a persona names (§1, #2039) was not ruled beyond §1 above.
The implementer decided it on 2026-10-15, to be confirmed by the operator:

- **It is an offer on the persona's tab, not an Ask.** Opening a persona is no wait: nothing is
  held until the person answers, and a chat handed to the persona meanwhile starts on the asking
  chat's profile and says so (D-1445-8). So the tab of a persona whose `profile:` names a profile
  this project does not offer on this machine carries a Notice, *Create profile <name>…*, and
  nothing is raised in the Inbox. A `model:` line names a model, and is never offered.
- **Nothing is made until the person presses Create.** The offer opens a form with the name fixed
  to the one the definition names and the harness to pick, and Create runs the command
  Settings › You › Profiles' Add profile runs, so the profile is made, imported and kept exactly
  as one made there. Signing it in is its own Ask (#2035), raised by the first chat on it.
- **The rules that stay hold for it as for any profile**: the project's dispatch list for the
  persona, the approval of a profile a clone's own local file declares under that name (which
  takes the place of the machine's), and the refusal of a command that switches the harness's
  permission prompts off. A profile made here runs its harness's own program with its folder
  variables set, so it carries no command to approve and no flag.
- **X12's acknowledgement** (§6) is #2168's, with its store as `docs/plane-format.md` records it
  and keyed as ADR 0087 keys it; a profile made here with a subscription is one of the logins it
  asks about.

## How a profile pins its harness version (#2038)

**Ruled by the operator on 2026-10-15**: a profile purlis made follows its harness's minor and
patch updates by itself, its pin moving and the chat's tab saying so, and only a new major
version refuses, with Update the pin as the way out; a conversation resumes on a version of the
same major as the one that wrote it, until a harness declaration says otherwise (#2160).

- **The pin is the version the profile's first chat finds installed**, as the harness answers
  `--version`, and is kept in the profile's record (`pinned`). No conversation is in a profile
  before its first chat. Adoption still runs no program.
- **A chat on a profile purlis made starts on its pinned version, or on a newer one of the same
  major, which the pin then follows.** On another major version, a new chat and a resumed one
  alike do not start, and the refusal names the way out: **Update the pin**, the person's act on
  the profile's page and the window's alone, or the pinned version installed again.
- **An adopted profile follows its harness** to any version, by the operator's condition that
  nothing is lost: its pin moves and the chat's tab says so. It never refuses a chat for its
  version.

Three readings were the implementer's on 2026-10-15, in implementation, to be confirmed by the
operator:

- **An older version than the pin, of the same major, is refused** for a profile purlis made,
  as another major is: the ruling follows updates, and an older version may not read what a
  newer one wrote.
- **A version purlis cannot read does not start a pinned profile purlis made**; a profile not yet
  pinned, and an adopted one, start as before.
- **Below 1.0, every release counts as a minor update and follows**, as Semantic Versioning
  numbers it (`0.147.0` to `0.148.0` keeps major version 0): a harness that releases weekly below
  1.0 (Codex) would otherwise ask the person to Update the pin every week.

## Amendments

The amendments to this decision are files of their own in [`0095/`](0095/), one per
amendment, each named by its date.
