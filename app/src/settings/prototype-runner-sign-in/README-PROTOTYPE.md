# PROTOTYPE: signing a profile in on a pool's runner

Throwaway. It lives on branch `prototype/runner-sign-in` and never merges to main.

## The question

What should signing a profile in on a pool's runner look like, for beginners and for advanced
users? The design agreed on 2026-10-30 (runner sign-in grilling, Q1 to Q12) is fixed. This
prototype only asks where the sign-in sits and how a person gets through it:

- No terminal on the default path. The window drives the vendor's login on the runner.
  - Claude Code: purlis opens the vendor's page in the browser on this Mac and shows a Paste
    code field, then Signed in.
  - Codex and opencode with ChatGPT: the browser's answer is forwarded to the runner, so the
    person approves and it finishes on its own.
  - Copilot: a device code, as in the window's GitHub sign-in dialog.
  - When purlis can't follow the login, the runner's live terminal shows inline in the same
    place. "Open in terminal" is a small secondary action.
- Each pool keeps its own profiles. The default is the profile a new chat in the open project
  would get. Profiles are added and removed on the pool's row.
- Sign-in is asked at the end of Add a pool, and at Move to runner when the chat's profile
  isn't signed in there. The move waits.
- A fresh cluster claim says its sign-in lasts only for this claim.

## Run it

```sh
cd app && pnpm storybook
```

Then open **PROTOTYPE › Runner sign-in › Variants** (`?path=/story/prototype-runner-sign-in--variants`).

The floating bar at the bottom switches:

- **Variant**: ← and → (or the bar's arrows).
- **State**: [ and ]: choose profiles, opening the browser, paste code (Claude Code), waiting
  for approval (Codex), device code (Copilot), signed in, couldn't follow (inline terminal),
  failed with the reason, fresh claim note.
- **Where**: Settings › Pools, the end of Add a pool, Move to runner.

The story's args hold the same three picks, so the URL keeps them and a link can be shared.
The bar is hidden in a production build. All data is stubbed: two pools (`dev-box` on Docker,
`team-gpu` on a fresh cluster claim) and three profiles (claude `work`, claude `personal`,
codex `oss`). The device-code state borrows a fourth profile, copilot `team`, since none of
the three signs in that way.

## The variants

- **A: inline in each pool's row.** Each pool's row lists its profiles. A profile's line
  expands in place into a sign-in strip. Profiles are picked with chips on the row. At Move to
  runner the strip opens under the chat's move Notice, in its pane.
- **B: one dialog per sign-in, in steps.** The row only shows how each profile stands. Each
  sign-in is its own dialog with numbered steps, one after another. Picking the profiles is a
  dialog of its own before them. Move to runner opens the same dialog over the chat.
- **C: a panel per pool, and one queue.** A pool's "Profiles on this pool" panel has a
  checkbox per profile beside a single "Sign in what's needed" queue that walks through each
  sign-in. Add a pool ends on that panel. Move to runner opens it under the move Notice with
  the chat's profile already ticked.

Files: `VariantA.tsx`, `VariantB.tsx`, `VariantC.tsx`, the shared vendor step
`SignInStep.tsx`, the host page `Frame.tsx`, the bar `PrototypeSwitcher.tsx`, stub data
`data.ts`, and the story `../PROTOTYPE-RunnerSignIn.stories.tsx`.

## Verdict (2026-10-30)

The person chose **Variant A, Inline**: a profile's line on the pool row opens in place into a sign-in strip; choosing profiles is a strip at the end of Add a pool; Move to runner shows the same strip under the chat's move notice. B and C are kept here as the primary source only.
