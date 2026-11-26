# Prerequisites: what a project needs from the person before its chats can work

**Accepted 2026-11-26**: the person's decisions on the prerequisite design, grilled for M75
(milestone 55) after #7714. It amends [ADR 0121](0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
([2026-11-26](0121/2026-11-26-an-open-prerequisite-may-interrupt-with-a-modal.md)) and
[ADR 0104](0104-the-window-is-built-from-component-families-on-react-aria-each-primitive-wrapped-once.md)
([2026-11-26](0104/2026-11-26-the-dialog-family-serves-an-app-raised-alert.md)), and replaces
`ProjectPolicyNotice` for two of its states under
[ADR 0112](0112-one-notice-for-one-need-a-person-may-allow-an-undeclared-program.md)'s one notice for
one need. It keeps [ADR 0125](0125-blocks-are-off-by-default-and-opt-in-per-family.md)'s rules on
who may loosen a project's policy whole.

## Where purlis was

A project's policy acceptance (ADR 0125 §3) has three readings on this machine
(`purlis_core::sandbox::local::PolicyAccepted`): **Never** (nothing kept, the shipped `watch`
stands), **Held**, and **Broken** (a record was kept and what is here is not it). Beside them the
policy may be **Waiting**: the committed file is looser than what this machine accepted.

- **Broken stops nearly everything and says almost nothing.** Every family is enforced at the full
  sandbox and no chat dispatches to another persona. The only sign is a Notice among the others
  (`app/src/ProjectPolicyNotice.tsx`) and the pane head's `strict` (`app/src/PolicyMark.tsx`).
  After an app update that macOS treats as a different program, every project reads Broken, and
  Accept cannot recover it (#7714). Engineers who updated purlis hit refusals with no
  explanation.
- **Refusals blame the wrong thing.** A dispatch refused because the acceptance is Broken says
  "purlis could not read the list of pairs you said never to" (`dispatchnever::unread_said`), and
  a guard's refusal under Broken or Waiting names its family, never that the policy is not
  accepted here.
- **The window decides when to look.** It reads `project_policy_state` on focus and on a change of
  the settings file, so the CLI, `purlis doctor` and the notifier see nothing of it, and another
  window learns of an Accept only when it is next focused.

## The decision

### 1. A prerequisite is the core's

A **prerequisite** is a condition a project must meet before its chats can work. `purlis-core`
defines each one, raises it, clears it, and pushes one **"prerequisites changed"** event, which the
window, the CLI and the notifier read. The window never decides a prerequisite itself, and stops
polling `project_policy_state` on focus for it. Several open prerequisites come in a fixed order
the core sets.

The mechanism is generic. It ships with **one prerequisite, the policy one**; a sign-in that
expired and a migration are later prerequisites, each with a decision of its own.

### 2. The policy prerequisite: Broken and Waiting

It is open while this machine's acceptance of the project's policy is **Broken** (acceptance does
not hold: every family enforced at the full sandbox, no cross-persona dispatch) or **Waiting**
(the committed policy is looser than this machine's acceptance). **Never**, the shipped `watch`
default, is not a prerequisite: it stays the one-time Notice `PolicyWatchAnnounced`.

### 3. When it is raised

On a project's open, on a policy change mid-session, and after an upgrade. Where the project is not
the one in front, a badge shows on its project chip, and the modal appears when the person switches
to it.

### 4. The modal

- **An alert dialog of the dialog family** (ADR 0104, as amended 2026-11-26), one at a time; several
  open prerequisites are shown one after another in the core's order.
- **Title:** "Before your chats can work in <project>". Then one line of why, and a summary of what
  changed since the last acceptance: families loosened or tightened, and an administrator's locks.
  A link opens the full policy view (Settings › Project › Policy).
- **Answers:** **Work in strict mode**, which has focus first, and **Accept**, of the `grants`
  variant, never pressed by Return. Where the seal key is foreign (#7714), Accept is **Make a new
  key and accept**.
- **Accept keeps today's rule:** it accepts exactly what was shown, bound to its digest; if the
  committed file, the local file or a lock changed since, nothing is accepted (`docs/policy.md`).
- **After Accept**, a news Notice: "Accepted. Running chats follow it from their next step."

### 5. What an open prerequisite holds

- **A new chat the person starts in that project waits** until they answer. It is not a **Hold**:
  nothing is turned into an ask.
- **Running chats keep running, in strict mode**, and a task one of them dispatches starts in strict
  mode as its parent runs.

### 6. Work in strict mode

The answer lasts until the app restarts or the policy changes. Meanwhile the prerequisite's
**Notice** stands, and cannot be dismissed while the prerequisite is open; its main action reopens
the modal. That Notice **replaces `ProjectPolicyNotice`** for Broken and Waiting (one notice for one
need, ADR 0112). The `strict` mark on the pane head stays. A new chat started after this answer
starts in strict mode.

### 7. Refusals name the cause

Every guard refusal that an open prerequisite causes says so, for example: "…refused because this
project's policy isn't accepted on this machine — accept it in purlis's window". The dispatch
refusal that blames "the list of pairs you said never to" for a Broken acceptance is fixed to name
the policy.

### 8. Not an ask

A prerequisite is never in **Asks** and adds nothing to the ✋ count. Its signals are its own: the
modal, the chip's badge, its Notice and the CLI's words.

- **`purlis doctor` and the CLI show the open prerequisites**, and a CLI start that waits on one
  prints why and where it is answered. **Only the window answers**: there is no CLI accept.
- **Answering in one window closes it in every window**, through the same event.
- **Acceptance stays per machine.** A pool or runner chat keeps the acceptance of the machine it was
  started from, and its refusals name the prerequisite and that machine's window.

### 9. Built in this order

1. **Quick relief:** one press for a new key (#7714), and refusals that name the cause (§7).
2. **Then:** the core's prerequisite list and its event with the policy prerequisite; the modal and
   the Notice; the chip's badge; the CLI and doctor; runners; the docs.

## Consequences

- ADR 0121's "nothing else interrupts" has one exception, an open prerequisite's modal (its
  amendment of 2026-11-26).
- The dialog family serves an alert the app raises with no press (ADR 0104's amendment of
  2026-11-26).
- `ProjectPolicyNotice` keeps only what is not a prerequisite: the policy purlis could not read
  (`policy-unreadable`). `PolicyWatchAnnounced` is unchanged.
- The window's focus polling of the policy state goes; the window, the CLI and the notifier all read
  the core's event, so they cannot disagree.
- A new chat may wait at its start for a reason that is not an ask, so every start path (window,
  CLI, dispatch, pool, runner) has to say why it waits.
- A stray key while the modal opens can only choose strict mode: focus starts there and Accept is
  never found by Return.
- Not chosen: a ✋ count for a prerequisite (it stops no running chat, and Asks would hold a
  question only the window can answer); a CLI accept (a chat could run it); the window deciding when
  a prerequisite is open (each surface would answer differently); a modal for **Never** (the shipped
  default is not a loosening the person must weigh).
