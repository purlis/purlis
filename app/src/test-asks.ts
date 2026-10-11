import type { ChatBlocked, DispatchPending, GrantLevel, Offered, Shown } from "./bindings";
import { inertly } from "./sandboxBlocks";

/**
 * **A stand-in for the asks registry, for window tests** (#1690, #1695): what `asking.rs`
 * derives for a block or a held dispatch the core holds, in its order and its words (#1700).
 * The pane's Notices draw from these, so a test that holds a block or a dispatch lists its ask
 * here too, as the core would.
 */

/** The registry's labels, as `asking.rs` writes them (`allow_said`). */
export const ALLOW_SAID = {
  sandbox: {
    you: "Allow for me on this machine",
    chat: "Allow only for this chat",
    project: "Allow for everyone in this project",
  },
  dispatch: {
    chat: "Allow for this chat",
    you: "Allow for me on this machine",
    project: "Allow for everyone in this project",
  },
} as const;

const allowing = (labels: Readonly<Record<GrantLevel, string>>, levels: readonly GrantLevel[]) =>
  levels.map((level): Offered => ({ id: level, label: labels[level], allows: true }));

const KEEP: Offered = { id: "keep", label: "Keep blocked", allows: false };
const NEVER: Offered = { id: "never", label: "Never for this pair", allows: false };

/** The ask the registry lists for `block`, while the core holds it: none for one that is no
 *  ask (purlis's own, or one no grant can name). */
export function askOfBlock(block: ChatBlocked, chain = [`chat ${block.session}`]): Shown[] {
  if (block.ours || (block.offer !== "host" && block.offer !== "write")) return [];
  const target = block.target ?? "";
  const levels: readonly GrantLevel[] =
    block.offer === "host" ? ["you", "chat", "project"] : ["chat", "you"];
  return [
    {
      session: block.session,
      ask: `block:${block.session}:${block.operation}:${block.kind}:${target}`,
      // Written out as the registry writes it (`asking::said_inertly`).
      says:
        block.offer === "host"
          ? `The sandbox refused ${inertly(target)}`
          : `The sandbox refused a write in ${inertly(target)}`,
      options: [...allowing(ALLOW_SAID.sandbox, levels), KEEP],
      source: block.offer === "host" ? "sandbox-host" : "sandbox-write",
      chain,
      answer:
        block.target === null
          ? { via: "in-its-pane" }
          : {
              via: "sandbox-block",
              shown: { operation: block.operation, kind: block.kind, what: block.offer, target },
            },
      held_until: block.held ? Math.floor(Date.now() / 1000) + 60 : undefined,
    },
  ];
}

/** The ask the registry lists for a dispatch held for the person. */
export function askOfDispatch(held: DispatchPending, chain = [held.chat]): Shown {
  const options =
    held.locked !== null
      ? []
      : [
          ...allowing(
            ALLOW_SAID.dispatch,
            (["chat", "you", "project"] as const).filter((level) => held.levels.includes(level)),
          ),
          KEEP,
          ...(held.asking === null ? [] : [NEVER]),
        ];
  return {
    session: held.session,
    ask: `dispatch:${held.id}`,
    says: `Wants to hand a task to ${held.target}`,
    options,
    source: "dispatch",
    chain,
    answer:
      options.length === 0
        ? { via: "in-its-pane" }
        : { via: "dispatch", id: held.id, shown: held.shown },
  };
}
