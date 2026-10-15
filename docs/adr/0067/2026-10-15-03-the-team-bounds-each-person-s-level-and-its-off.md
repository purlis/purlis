# Amended (2026-10-15, #2071 PR C; the operator's rulings SO-5, SO-8 and SO-9 of 2026-10-14, and of 2026-10-15 on the spelling; amends section 5): the team bounds each person's level, and its off reaches each person on their Accept

Ruled by the operator. Section 5's "a committed file may turn the sandbox on and never off" is
amended (SO-5): the team decides, in the committed `charter.toml`, how far its people may turn
the sandbox down.

- **`[sandbox] policy = "local" | "never"`** (absent is `local`).
  - `local`: each person's own level on their machine holds (`on`, `floor` or `off`, ADR 0096),
    as far as they confirmed it; the floor where they chose none (SO-9).
  - `never`: nobody runs below the full sandbox. "Start without the sandbox" is refused on every
    chat, and a person's own lower level is refused when set and clamped to the full sandbox when
    a chat starts, each with a sentence naming the policy. It is carried as the administrator's
    opt-out lock is (`policy::Locks::of`), and said as the project's.
  - A value that is neither is refused with a sentence and read as `never`: a typo never loosens
    anyone's sandbox.
- **`[sandbox] mode = "off"` is the team's off** (the operator, 2026-10-15: the line a project
  already carries, no new value). It reaches each person only by consent (SO-8): the first time
  a commit turns it off, the window asks once, with **Accept** and **Keep it on for me**. Until
  they answer, their chats keep their own level, or the floor. Accepted, a chat whose person
  chose no level runs with no sandbox from its next start; a level they chose still stands.
  Settings › Sandbox › Your level shows the answer and offers the other one.
  - Only a committed off asks anyone: an off in the working tree alone turns nothing off. The
    commit is the one whose `[sandbox]` turned it off, read from the history by what that table
    says (never a `mode` of another table).
  - **What the ask says rests only on what purlis verified itself** (#2156). It names the commit
    and its author, "Your project turned the sandbox off (commit <sha> by <author>)", only where
    that commit is in the history of the upstream commit purlis's own fetch of the project
    found, read as written (no replacement objects or grafts). That commit is the remote's own
    answer to the app's fetch, kept in this machine's config home; a ref in the clone, a
    remote-tracking one included, is never taken as proof. Otherwise the ask says the off is
    local: "This project's sandbox off is local: it is not on the upstream purlis fetched."
  - **The answer is kept for the off as purlis saw it, not for a commit** (#2156), in this
    machine's config home (`sandbox-grants`), which no sandboxed chat writes. A history rewritten
    under an off that stays asks nobody again. purlis lets the answer go when it reads the
    project's file not turning the sandbox off (`mode` changed, or `policy = "never"`), so only
    an off purlis itself saw leave and come back is asked again.
  - Under `policy = "never"`, `mode = "off"` is refused with a sentence naming the policy, and
    the sandbox stays on.
- **One bound for every start**: `sandbox::team::allowed_levels(root)` is what a person's level
  is clamped to, and `team::level_for(root, persona)` the level a chat runs at. A chat's start
  reads it through the one seam #2122 adds (`sandbox::bounded`).
- **What stays**: the harness's own permission prompts, and purlis's guards of the project root
  and its own state, at every level.
