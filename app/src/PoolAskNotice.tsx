// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PlaneId, Shown } from "./bindings";
import { OneAsk } from "./BrowserAskNotice";

/** The asks of a chat on a pool's runner its pane draws here, by the path each is answered
 *  on, and the label its Notice is named. */
const DRAWN: Readonly<Partial<Record<Shown["answer"]["via"], string>>> = {
  // Its browser asks to be logged in with its persona's logins on the pool's runner (#2050).
  "pool-login": "Browser login on a pool",
  // One run of its declared tool wants its vault values on the pool's runner (#2047).
  "pool-tool": "Tool run on a pool",
};

/**
 * **What a chat on a pool's runner asks the person, on its pane** (plane #2697): a login for
 * its browser there, and one run of its tool with its vault values there. The chat waits on the
 * answer, so it is drawn where the person looks at the chat, not only in Asks.
 */
export function PoolAskNotice({
  plane,
  session,
  asks,
  onWay,
}: {
  plane: PlaneId;
  session: number;
  asks: readonly Shown[];
  onWay?: (answer: string, ask: Shown) => void;
}) {
  return (
    <>
      {asks.map((ask) => {
        const label = DRAWN[ask.answer.via];
        if (ask.session !== session || label === undefined) return null;
        return <OneAsk key={ask.ask} plane={plane} ask={ask} label={label} onWay={onWay} />;
      })}
    </>
  );
}
