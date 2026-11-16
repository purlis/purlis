// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { Unlock } from "./bindings";
import { Notice } from "./Notice";
import { windowCopy } from "./windowCopy";

/** What the Ask is doing: waiting on the press, unlocking, or after a failed unlock. */
type Stage = "waiting" | "unlocking" | "failed";

/**
 * **Your keys locked at a start, in place in New chat** (T1a-61, R-510, R-592): W10, *"Your
 * keychain is locked. Unlock it to start this chat."*, with *Unlock*, while the person is there.
 * (An unattended start's Ask goes to Needs you instead, drawn by Asks' decision row.)
 *
 * *Unlock* runs the OS's unlock (`unlock`, the `unlock_your_keys` command) and is busy while it
 * runs. Where it unlocks, the start goes on (`onUnlocked`), with no second press; a cancel leaves
 * W10 as it was; a failure puts W84, *"purlis couldn't unlock your keychain."*, under it, and
 * *Unlock* tries again. Neither starts the chat.
 */
export function UnlockAsk({
  entry,
  unlock,
  onUnlocked,
}: {
  /** Your keys' entry the start waits on: what the Ask is keyed by. */
  entry: string;
  unlock: () => Promise<Unlock>;
  onUnlocked: () => void;
}) {
  const [stage, setStage] = useState<Stage>("waiting");
  const press = () => {
    if (stage === "unlocking") return;
    const was = stage;
    setStage("unlocking");
    unlock().then(
      (said) => {
        if (said === "unlocked") {
          setStage("waiting");
          onUnlocked();
        } else if (said === "failed") {
          setStage("failed");
        } else {
          setStage(was);
        }
      },
      () => setStage("failed"),
    );
  };
  return (
    <Notice
      cause={`model-key-locked:${entry}`}
      tone="attention"
      waits
      fixes={[{ label: "Unlock", onPress: press, busy: stage === "unlocking", primary: true }]}
      under={stage === "failed" ? <p className="unlock-ask-failed">{windowCopy.W84}</p> : undefined}
    >
      {windowCopy.W10}
    </Notice>
  );
}
