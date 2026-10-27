// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ChatBlocked, DispatchPending, GrantLevel, Offered, Shown } from "./bindings";
import { inertly, isGrantedOffer } from "./sandboxBlocks";
import {
  ALLOW_SAID as DISPATCH_ALLOW_SAID,
  KEEP_SAID,
  NEVER_SAID,
  SANDBOX_ALLOW_SAID,
} from "./allowLabels";

/**
 * **A stand-in for the asks registry, for window tests** (#1690, #1695): what `asking.rs`
 * derives for a block or a held dispatch the core holds, in its order and its words (#1700).
 * The pane's Notices draw from these, so a test that holds a block or a dispatch lists its ask
 * here too, as the core would.
 */

/** The registry's labels, as `asking.rs` writes them (`allow_said`): `allowLabels.ts`'s. */
export const ALLOW_SAID = { sandbox: SANDBOX_ALLOW_SAID, dispatch: DISPATCH_ALLOW_SAID } as const;

const allowing = (labels: Readonly<Record<GrantLevel, string>>, levels: readonly GrantLevel[]) =>
  levels.map((level): Offered => ({ id: level, label: labels[level], allows: true }));

const KEEP: Offered = { id: "keep", label: KEEP_SAID, allows: false };
const NEVER: Offered = { id: "never", label: NEVER_SAID, allows: false };

/** The ask the registry lists for `block`, while the core holds it: none for one that is no
 *  ask (purlis's own, or one no grant can name). */
export function askOfBlock(block: ChatBlocked, chain = [`chat ${block.session}`]): Shown[] {
  const offer = block.offer;
  if (block.ours || !isGrantedOffer(offer)) return [];
  const local = offer === "socket" || offer === "service";
  const target = block.target ?? "";
  const levels: readonly GrantLevel[] =
    offer === "host"
      ? ["you", "chat", "project"]
      : offer === "port" || offer === "listen"
        ? // The registry's order (`asking::sandbox_port`).
          ["chat", "you"]
        : local
          ? // The registry's order (`asking::sandbox_local`): the narrowest, then every project.
            ["once", "machine", "chat", "you"]
          : ["chat", "you"];
  return [
    {
      session: block.session,
      ask: `block:${block.session}:${block.operation}:${block.kind}:${target}`,
      // Written out as the registry writes it (`asking::said_inertly`).
      says:
        offer === "host"
          ? `The sandbox refused ${inertly(target)}`
          : offer === "port"
            ? target === ""
              ? "The sandbox refused a service on this Mac at a port it did not name"
              : `The sandbox refused the service on this Mac at port ${target}. ${block.route ?? ""}`
            : offer === "listen"
              ? target === ""
                ? "This chat wants to run a server on a port it did not name"
                : `This chat wants to run a server on port ${target}. ${block.route ?? ""}`
              : local
                ? `The sandbox refused the ${offer === "socket" ? "local socket" : "system service"} ${inertly(target)}. ${block.route ?? ""}`
                : `The sandbox refused a write in ${inertly(target)}`,
      options: [...allowing(ALLOW_SAID.sandbox, levels), KEEP],
      source:
        offer === "host"
          ? "sandbox-host"
          : local || offer === "port" || offer === "listen"
            ? "sandbox-local"
            : "sandbox-write",
      chain,
      answer:
        block.target === null
          ? { via: "in-its-pane" }
          : {
              via: "sandbox-block",
              shown: { operation: block.operation, kind: block.kind, what: offer, target },
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
    says:
      held.mode === "handoff"
        ? `Wants to hand its work off to ${held.target}`
        : `Wants to hand a task to ${held.target}`,
    options,
    source: "dispatch",
    chain,
    answer:
      options.length === 0
        ? { via: "in-its-pane" }
        : { via: "dispatch", id: held.id, shown: held.shown },
  };
}
