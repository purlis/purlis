// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Button } from "../ui/button/Button";
import { Say } from "./parts";
import { type PoolState, type PoolStatus } from "./poolState";

/**
 * **One status bar at the top of Pool setup** (#7191): the pool's state and what can be done about
 * it, in one place. Window notices and Settings › Pools only point here. The five names are the
 * core's ([`poolState.ts`](./poolState.ts)); a forgotten pool has nothing to show.
 */

const TITLE: Record<PoolState, string> = {
  unreachable: "Unreachable",
  refused: "Refused",
  "host-key-changed": "Host key changed",
  "runner-gone": "Runner gone",
  "pool-forgotten": "Pool forgotten",
};

const TONE: Record<PoolState, "attention" | "trouble"> = {
  unreachable: "attention",
  refused: "trouble",
  "host-key-changed": "trouble",
  "runner-gone": "attention",
  "pool-forgotten": "attention",
};

function headline(status: PoolStatus, machine: string): string {
  switch (status.state) {
    case "unreachable":
      return `purlis can't reach ${machine} and keeps trying.`;
    case "refused":
      return `${machine} answers, and refused: ${status.reason ?? "no reason was given"}. purlis stopped trying.`;
    case "host-key-changed":
      return `${machine} shows a different host key. purlis stopped and waits for your OK.`;
    case "runner-gone":
      return `The runner on ${machine} is gone. Its home volume, with your sign-ins, is still there.`;
    case "pool-forgotten":
      return "Forgotten on this desktop.";
  }
}

export function PoolStatusBar({
  pool,
  machine,
  status,
  onTryNow,
  onCheckPool,
  onCheckKey,
  onRemake,
  onForget,
}: {
  pool: string;
  /** What the pool is on, as the person named it: a host, or a cluster's context. */
  machine: string;
  status: PoolStatus;
  onTryNow: () => void;
  onCheckPool: () => void;
  onCheckKey: () => void;
  onRemake: () => void;
  onForget: () => void;
}) {
  if (status.state === "pool-forgotten") return null;
  return (
    <section aria-label={`Pool ${pool}`} className="pool-setup-statusbar">
      <Say
        tone={TONE[status.state]}
        title={`${TITLE[status.state]}: pool ${pool}`}
        actions={
          <>
            {status.state === "unreachable" && (
              <Button variant="primary" onPress={onTryNow}>
                Try now
              </Button>
            )}
            {status.state === "refused" && (
              <Button variant="primary" onPress={onCheckPool}>
                Check pool
              </Button>
            )}
            {status.state === "host-key-changed" && (
              <Button variant="primary" onPress={onCheckKey}>
                Check the new key
              </Button>
            )}
            {status.state === "runner-gone" && (
              <Button variant="primary" onPress={onRemake}>
                Make the runner again
              </Button>
            )}
            <Button variant="secondary" onPress={onForget}>
              Forget pool…
            </Button>
          </>
        }
      >
        <p>{headline(status, machine)}</p>
      </Say>
    </section>
  );
}
