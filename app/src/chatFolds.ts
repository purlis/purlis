import { keepChatsPart, keptChatsPart } from "./chatsScope";

/**
 * **The folds the person set on the Chats list, kept across a reload and a relaunch** (#1459,
 * V100-48: "a hand-set fold is kept"; #1687).
 *
 * Two places, for two lifetimes:
 *
 * - **By chat id, in `layout.json` v2** (B-11, #1687): the project's `chats.folds`, beside the
 *   Chats scope and Explorer's folds (`projectViews.ts`). A chat's id (ADR 0066, `OpenChat.id`)
 *   is what it keeps across a launch, so a relaunch draws the list folded as it was left. An
 *   open chat's fold replaces what the file held for it; a chat not open now keeps its fold,
 *   since a launch lists its chats one by one and a finished task reopened keeps its id. The
 *   entry is held to {@link MOST_FOLDS}: the folds of chats not open now go first, the ones
 *   kept longest ago first.
 * - **By chat number, in the window's session storage**, for a chat the core gave no id: a
 *   number holds for as long as the app runs, so a reload of the window keeps its fold and a
 *   launch starts it from the folds the list makes by itself.
 *
 * Nothing here is needed: storage a webview refuses, or a value that is not one, is no fold.
 */

const KEY = "purlis.chats.folds:";

/** The most folds a project keeps in the layout file: a ULID and its answer are some forty
 *  bytes written out, so the entry stays under two KiB, as Explorer's does. */
export const MOST_FOLDS = 48;

/** A chat id as the core mints one (ADR 0066): a ULID, 26 characters of Crockford's base 32.
 *  Nothing else is read back from the file, so a hand-edited key cannot grow the entry. */
const CHAT_ID = /^[0-9A-HJKMNP-TV-Z]{26}$/;

/** The folds kept for `plane`, by chat number: true is folded. */
export function keptFolds(plane: string | undefined): ReadonlyMap<number, boolean> {
  const folds = new Map<number, boolean>();
  if (plane === undefined) return folds;
  try {
    const raw: unknown = JSON.parse(globalThis.sessionStorage.getItem(KEY + plane) ?? "null");
    if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return folds;
    for (const [session, shut] of Object.entries(raw as Record<string, unknown>)) {
      const number = Number(session);
      if (Number.isInteger(number) && number > 0 && typeof shut === "boolean")
        folds.set(number, shut);
    }
  } catch {
    // No storage, or nothing readable in it: no fold.
  }
  return folds;
}

/** The folds the layout file keeps for `plane`, by chat id: true is folded. Read as names and
 *  answers and nothing else. */
export function keptFoldsById(plane: string | undefined): ReadonlyMap<string, boolean> {
  const folds = new Map<string, boolean>();
  if (plane === undefined) return folds;
  const held = keptChatsPart(plane, "folds");
  if (held === null || typeof held !== "object" || Array.isArray(held)) return folds;
  for (const [id, shut] of Object.entries(held as Record<string, unknown>)) {
    if (CHAT_ID.test(id) && typeof shut === "boolean") folds.set(id, shut);
  }
  return folds;
}

/**
 * Keeps `folds` for `plane`, in place of what was kept: every one by its number for a reload of
 * the window, and each chat `listed` names an id for by that id in the layout file. A listed
 * chat with no fold set by hand has none kept; a chat not listed keeps what the file held for
 * it, so the folds of chats a launch has not listed yet are not lost before they are.
 */
export function keepFolds(
  plane: string | undefined,
  folds: ReadonlyMap<number, boolean>,
  listed: ReadonlyMap<number, string> = new Map(),
): void {
  if (plane === undefined) return;
  try {
    if (folds.size === 0) globalThis.sessionStorage.removeItem(KEY + plane);
    else globalThis.sessionStorage.setItem(KEY + plane, JSON.stringify(Object.fromEntries(folds)));
  } catch {
    // Storage refused: the fold holds for this window only, as it did before.
  }
  if (listed.size === 0) return;
  const byId = new Map(keptFoldsById(plane));
  for (const [session, id] of listed) {
    byId.delete(id);
    const shut = folds.get(session);
    if (shut !== undefined) byId.set(id, shut);
  }
  const kept = [...byId].slice(-MOST_FOLDS);
  keepChatsPart(plane, "folds", kept.length === 0 ? undefined : Object.fromEntries(kept));
}

/**
 * The folds `plane`'s file keeps for the chats in `listed` that `adopted` has not taken yet, by
 * their numbers now: what a launch puts back into the folds set by hand as each chat is listed.
 */
export function foldsToAdopt(
  plane: string | undefined,
  listed: ReadonlyMap<number, string>,
  adopted: ReadonlySet<string>,
): ReadonlyMap<number, boolean> {
  const out = new Map<number, boolean>();
  const byId = keptFoldsById(plane);
  for (const [session, id] of listed) {
    if (adopted.has(id)) continue;
    const shut = byId.get(id);
    if (shut !== undefined) out.set(session, shut);
  }
  return out;
}
