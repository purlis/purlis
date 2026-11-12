# PROTOTYPE: Pool setup's Connect step for Docker on another machine (#6265)

Throwaway. It lives on branch `prototype/pool-setup-remote-docker` and never merges to main.
Parent: #6257, ADR 0123 §6 (PR #6263). It follows #4469's accepted prototype (`prototype/pool-setup`).

## The question

Can someone who has never set up Docker on another machine get through Connect alone, in every
state ADR 0123 names? #4469's layout is accepted, so this draws one design in full, not variants.

## Open it

`cd app && npm ci && npm run storybook`, then **PROTOTYPE › Pool setup, another machine**.

- **Click through** starts at Where. Every button walks on.
- Every other story is one state, and stays live from there.
- **Light and dark**: the toolbar's Appearance (and Theme) switch it.
- The strip marked Prototype at the top of the page is not the design. It sets what the other
  machine does when purlis checks it, and whether your ssh already knows the machine's key.

All data is made up. Nothing contacts Docker, ssh or another machine.

## States, each a story

Where: Docker here, no Docker here (another machine recommended), Docker pointing elsewhere
(`Engine::Elsewhere` with its second action). Connect: the route (SSH preselected, TLS API, Docker
context, Podman connection), the machine picked or typed (and a typo), plain TCP refused, asking
for the key, first contact (a key your ssh knows, a new key, a TLS authority), Trust this machine,
the engine check running, each §7 finding, every check passed with the owned-mark question. The pool
reopened: the pin-changed Notice (machine key, authority, Docker engine) and doctor's context
finding.

## UX choices settled here, for acceptance

1. **Connect is one page that grows, not sub-screens.** Route and machine fold into a facts card
   with Change once purlis contacts the machine. The step list stays the six steps of §6, and
   the person always sees what they chose.
2. **"Connect to build-box" comes before Trust this machine.** Purlis contacts another machine
   only when the person presses a button (Q31), and a typed address can't be read while it is being
   typed. The button says it only reads and keeps nothing. Then the main button is Trust this
   machine, then Continue (§6).
3. **The found machines are a visible radio list, with "Another machine" last, not a combo box.**
   A newcomer sees what purlis found and where each came from ("from your SSH settings") without
   opening anything. The typed field stands under the list, not inside a radio (#6206).
4. **A context or connection that isn't a route is still listed.** A local context is shown
   disabled with "Choose This machine under Where". A plain-TCP context can be picked, and picking
   it leads to the refusal, so the person learns why rather than wondering where it went.
5. **The plain-TCP refusal is an attention card, not a trouble card.** Nothing broke: purlis
   declined on purpose. Its main way forward is Use SSH instead (fills in the same host). Turn on
   TLS links to Docker's guide, with the commands under See what this does.
6. **The fingerprint is a code block with Copy, and See what this does opens by itself for a new
   key or an authority.** Comparing the key is the point of first contact. When your ssh already
   knows the key, it stays folded, since that check already happened.
7. **A second way out, "This isn't it"**, beside Trust this machine, goes back to the pick. That
   way the person never has to trust a key just to leave.
8. **Each §7 finding is the check list with the failing line marked, then one trouble card**: a
   plain title, the ADR's sentence, a plain-words line where there is jargon (agent, socket,
   docker group, CA, seccomp), one main remedy, and Change the machine. Commands that are run on
   the other machine sit under See what this does, open. Checks that are not about the route
   (sign-in for TLS, certificate for SSH) are left out of the list.
9. **The owned mark is a two-option radio worded as a question about people**: "Who can sign in to
   build-box.lan?" The recommended answer is "Others might, or I'm not sure". The word "owned" is
   never shown: what it means (your saved logins may reach it) is.
10. **The pin-changed Notice is a trouble card**, with Check and trust again (primary) and Remove
    pool. It shows what was trusted and what it shows now. For an engine change, Check and trust
    again reruns the engine check rather than the key confirmation, since the machine's key still
    holds.
11. **Doctor's context finding is an attention card on the pool's page**, with Connect again or
    Keep it as it is. It is not a Notice, since the pool still runs safely.

## Verdict

(Awaiting acceptance on #6265.)
