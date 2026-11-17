# Amended 2026-11-17: a GitHub App's wait ends on a code that isn't its own

Proposed by the steward on 2026-11-17, from a person's report, for the person's ruling in the pull
request that carries it. It changes the Agent identity flow (the amendment of 2026-10-29) and the
loopback listener of §3 only for that flow. The browser sign-ins of §3 are as they were.

## What happened

A person made the App on GitHub's page and GitHub sent their browser back to the flow's address
with a code. The listener answered *Nothing to see here*, and the window said *Waiting for GitHub*
until the 15 minutes ran out, with no word of the App GitHub had already made. A loop at the
listener's seam finds two causes that look the same from outside: a request head longer than the
listener read (8 KB, which a browser passes with the cookies every local server has set for
127.0.0.1), and a code without this try's `state`. purlis can't tell which reached the person: the
listener logged nothing.

## What changes

- **A request to the flow's own redirect address that carries a code, without this try's
  `state`, ends the wait.** The code is not taken. The browser is told to return to purlis, and
  the window says a code came back that purlis couldn't match, that pressing Create GitHub App made
  the App, where the owner's Apps are on GitHub, and how to finish. Before, it was refused like any
  stranger and the wait went on, unseen.
- **The person can finish from the address the browser ended on.** They paste it in the window.
  Its code is converted without the `state`, since the person brought it, and the answer is held
  to what was asked as any other: the owner named, the permissions, no events. A pasted address
  that isn't the redirect sends nothing and stops nothing.
- **A request's head may be 32 KB** (nginx's default), for every loopback listener.
- **Each refusal is logged by its fixed reason**, never with anything of the request.

## What it costs

One request to the flow's address with any code now ends a wait that 32 such requests ended
before. Doing that takes the listener's port, which the system picks for each try. The worst it
does is end that try: no code a stranger sends can get an App kept, since the conversion is held to
the owner the person chose, and a real code that reaches a closed listener can still be pasted.
