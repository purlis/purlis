// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Why a window test's chats have no pane, said where it fails** (#4758). Since the switch a
 * launch draws tabs from the stored layout only (`reopened_layout`): a chat the core reopens
 * (`opened_chats`) with no tab in that layout runs on in the sidebar, and no pane shows it. A
 * test written the old way, its stand-in answering the chats and not the layout, fails on a pane
 * that never came ("Unable to find an element by: [data-testid="pane"]"), which says nothing
 * of the cause; window tests main gained after the migration kept arriving that way.
 *
 * So every stand-in is watched (`mockIPC`, wrapped in `test-setup.ts`), and a test that fails
 * after its stand-in reopened chats and never laid them out is told so, with the fix. A test
 * that passes is never touched: a chat in the sidebar only is a state some tests want.
 */

/** What the current test's stand-in has answered, as far as the guard cares. */
let heard = { chats: 0, laid: false };

/** Forgets what the last test's stand-in answered. */
export function forgetStandIn(): void {
  heard = { chats: 0, laid: false };
}

/** Notes one answer of a stand-in. */
function note(cmd: string, answer: unknown): void {
  if (cmd === "opened_chats" && Array.isArray(answer))
    heard.chats = Math.max(heard.chats, answer.length);
  if (cmd === "reopened_layout" && answer !== null && answer !== undefined) heard.laid = true;
}

/** `handler`, noting what it answers, and answering exactly what it does. */
export function watched<H extends (cmd: string, ...rest: never[]) => unknown>(handler: H): H {
  const answering = handler as unknown as (cmd: string, args: unknown) => unknown;
  return ((cmd: string, args: unknown) => {
    const answer = answering(cmd, args);
    if (answer instanceof Promise)
      return answer.then((value: unknown) => {
        note(cmd, value);
        return value;
      });
    note(cmd, answer);
    return answer;
  }) as unknown as H;
}

/**
 * What a failed test is told, or nothing: a stand-in that reopened chats and laid none of them
 * out. Read after the test, so only once it has failed.
 */
export function whyNoPane(): string | undefined {
  if (heard.chats === 0 || heard.laid) return undefined;
  return (
    `this test's stand-in core answered opened_chats with ${heard.chats} chat(s) and never ` +
    "answered reopened_layout. Since #4758 a launch draws tabs from the stored layout only, so " +
    "those chats run in the sidebar and no pane shows them. If the test wants them on the " +
    'strip, answer `if (cmd === "reopened_layout") return aTabForEachChat(chats, …)` ' +
    "(laidOut.testkit.ts)."
  );
}
