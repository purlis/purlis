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
  // Not "Refused": a stop is "blocked" in the window's words (ui-copy R-170), and this is no
  // stop of purlis's: the machine or cluster would not take the pool.
  refused: "Not added",
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

/** Where the pool is: a machine with Docker, or a cluster's kubeconfig context. */
export type Where = { kind: "machine"; host: string } | { kind: "cluster"; context: string };

function headline(status: PoolStatus, where: Where): string {
  const cluster = where.kind === "cluster";
  const name = cluster ? `the cluster (context ${where.context})` : where.host;
  switch (status.state) {
    case "unreachable":
      return `purlis can't reach ${name} and keeps trying.`;
    case "refused": {
      const why = status.reason ?? "no reason was given";
      return cluster
        ? `${capital(name)} did not take the pool: ${why}. purlis stopped trying.`
        : `${name} answered, and the pool was not added: ${why}. purlis stopped trying.`;
    }
    case "host-key-changed":
      return `${name} shows a different host key. purlis stopped and waits for your OK.`;
    case "runner-gone":
      return `The runner on ${name} is gone. Its home volume, with your sign-ins, is still there.`;
    case "pool-forgotten":
      return "Forgotten on this desktop.";
  }
}

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

export function PoolStatusBar({
  pool,
  where,
  status,
  onTryNow,
  onCheckPool,
  onCheckKey,
  onRemake,
  onForget,
}: {
  pool: string;
  /** What the pool is on: a host the person named, or a cluster's kubeconfig context. */
  where: Where;
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
        <p>{headline(status, where)}</p>
      </Say>
    </section>
  );
}
