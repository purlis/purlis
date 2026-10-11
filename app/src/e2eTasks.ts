/// <reference types="vite/client" />
import { useEffect, useState } from "react";
import type { OpenChat } from "./bindings";

/**
 * **Tasks the scenario specs pretend a session has** (#1487), so a tab's chip and its menu
 * can be driven in the real WebView: what a pointer crossing the strip opens, what a rest
 * opens, how wide a tab is with a chip on it.
 *
 * A task needs a persona chat and a core that dispatched it, which the specs' fake harness
 * cannot make (`pane-crumbs.e2e.ts` says the same of the breadcrumb). The breadcrumb is only a
 * shape, and is drawn by its spec; a chip is what it does under a pointer, which only the
 * component does. So the spec says which chats to list as tasks, and the window lists them
 * beside the core's own.
 *
 * **It exists only in the `e2e` build**: `VITE_E2E` is set by `vite build --mode e2e` alone
 * (`bench.ts` is the other such seam), and in every other build this hears nothing and
 * answers the same empty list for ever.
 */
export const PRETEND_TASKS = "purlis-e2e-tasks";

/** One pretended task: the chat as the core would list it, and the workspace it works in. */
export type Pretended = { chat: OpenChat; workspace: string };

const NONE: readonly Pretended[] = [];

export function usePretendTasks(): readonly Pretended[] {
  const [tasks, setTasks] = useState(NONE);
  useEffect(() => {
    if (!import.meta.env.VITE_E2E) return;
    const hear = (event: Event) => {
      const said: unknown = (event as CustomEvent<unknown>).detail;
      setTasks(Array.isArray(said) ? (said as Pretended[]) : NONE);
    };
    window.addEventListener(PRETEND_TASKS, hear);
    return () => window.removeEventListener(PRETEND_TASKS, hear);
  }, []);
  return tasks;
}

/**
 * **The limit numbers the scenario specs pretend a session has** (#1545): how many tasks it may
 * run, how many it runs, and the limit that binds it, as the core would send them on the
 * session's own chat (`OpenChat.tasks_limit`, `tasks_running`, `at_limit`). Its tab menu's
 * footer says them (`taskLimits.ts`), so a spec can measure the longest footer there is,
 * `2 of 6 running · past its token limit (1.2M of 1M) · not enforced yet`, in the real WebView.
 *
 * The detail is a list of `{ session, ...numbers }`; an empty list takes them all away. As with
 * the tasks, only the `e2e` build hears it.
 */
export const PRETEND_LIMITS = "purlis-e2e-limits";

/** The numbers pretended for one session, as the core sends them on its chat. */
export type PretendedLimits = Pick<OpenChat, "tasks_limit" | "tasks_running" | "at_limit">;

const NO_LIMITS: ReadonlyMap<number, PretendedLimits> = new Map();

/** The numbers each session is pretended to have, by its number. */
export function usePretendLimits(): ReadonlyMap<number, PretendedLimits> {
  const [limits, setLimits] = useState(NO_LIMITS);
  useEffect(() => {
    if (!import.meta.env.VITE_E2E) return;
    const hear = (event: Event) => {
      const said: unknown = (event as CustomEvent<unknown>).detail;
      setLimits(
        Array.isArray(said)
          ? new Map(
              (said as ({ session: number } & PretendedLimits)[]).map(({ session, ...numbers }) => [
                session,
                numbers,
              ]),
            )
          : NO_LIMITS,
      );
    };
    window.addEventListener(PRETEND_LIMITS, hear);
    return () => window.removeEventListener(PRETEND_LIMITS, hear);
  }, []);
  return limits;
}

/** `chat` as the core would list it with the numbers pretended for it, where there are any:
 *  the one call the window makes on each chat it lists. */
export function withPretendedLimits(
  chat: OpenChat,
  limits: ReadonlyMap<number, PretendedLimits>,
): OpenChat {
  const pretended = limits.get(chat.session);
  return pretended === undefined ? chat : { ...chat, ...pretended };
}
