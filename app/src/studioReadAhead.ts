// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "./bindings";
import { followViewChanged } from "./planeChanged";
import { scopeKey, whoseChats, type StudioScope } from "./skillStudio";

/**
 * **The studio's skills, asked for as it is opened** (#4351, L10). Reading a project's skills is
 * most of an opening (some 280 ms with two hundred of them), and the tab used to ask only once it
 * was drawn, so the read waited for the draw. Opening the studio now asks at once, and the tab it
 * opens takes that read for its first: the two run side by side.
 *
 * **One read, for one tab's first read.** It is the tab's by the tab's history and the scope it
 * was opened on, and it is dropped as soon as that tab has drawn it, so no later read, by that
 * tab or another, is ever handed it. A change on disk the tab would read again for, heard before
 * it took the read, leaves the tab to read for itself: the read may be older than the change.
 */

type StudioAnswer = Awaited<ReturnType<typeof commands.skillStudio>>;

/** The read asked as the studio was last opened, until the tab it was asked for draws it. */
let ahead:
  | { tab: string; scope: string; answer: Promise<StudioAnswer>; stale: boolean; stop: () => void }
  | undefined;

/** **Asks for `scope`'s skills as the studio tab whose history is `tab` is opened on it.** */
export function readStudioAhead(plane: string, tab: string, scope: StudioScope): void {
  forgetStudioReadAhead();
  const { workspace, persona } = whoseChats(scope);
  const answer = commands.skillStudio(plane, workspace, persona);
  // A read no tab takes fails unheard: the tab that takes one says what it said.
  answer.catch(() => undefined);
  const read = { tab, scope: scopeKey(scope), answer, stale: false, stop: () => {} };
  // What the studio's view reads again for (`Views.tsx`, `followed`).
  read.stop = followViewChanged([plane], undefined, () => {
    read.stale = true;
  });
  ahead = read;
}

/**
 * **The read asked as tab `tab` was opened on the scope keyed `scope`**, for its first read, while
 * nothing heard since concerns it; otherwise nothing, and the tab reads for itself. Asked again
 * before it is drawn (React's strict mode draws a tab twice) it is the same read.
 */
export function studioOpeningRead(tab: string, scope: string): Promise<StudioAnswer> | undefined {
  const read = ahead;
  return read !== undefined && read.tab === tab && read.scope === scope && !read.stale
    ? read.answer
    : undefined;
}

/** **The tab has drawn `answer`**: when it was the opening's read, nobody is handed it again. */
export function studioOpeningReadDrawn(answer: Promise<StudioAnswer>): void {
  if (ahead?.answer === answer) forgetStudioReadAhead();
}

/** Drops the read asked as the studio was last opened, if any tab has not drawn it. */
export function forgetStudioReadAhead(): void {
  ahead?.stop();
  ahead = undefined;
}
