// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "./here";

import {
  commands,
  ENGINE_SANDBOX_NEEDED,
  type EngineMachine,
  type EngineSandboxShown,
} from "./bindings";
import { Notice } from "./Notice";

/**
 * **Browsers cannot start because the engine cannot make its sandbox here** (#2835, ruled
 * 2026-10-20).
 *
 * On Ubuntu 23.10 and later the pinned engine makes its sandbox only under an AppArmor profile
 * that names it, which the `.deb` installs (`enginesandbox.rs`). Where none names this machine's
 * engine, the session host refuses every browser, the chat is told a browser is not available
 * here, and this Notice offers the two ways out: the command that adds the profile, which the
 * person runs themselves, and using this machine's own Chrome for every browser where one is
 * installed. It is drawn once a browser was refused, never before.
 *
 * A pool's runner whose engine cannot make its sandbox has a Notice of its own (#2990), naming
 * the runner and offering the command to run there; this machine's Chrome is not offered for it.
 */
/** A machine's key among the Notices: this machine's, or a pool's runner's. */
function keyOf(machine: EngineMachine): string {
  return machine.kind === "here" ? "here" : `runner:${machine.pool}`;
}

export function EngineSandboxNotice() {
  const [shown, setShown] = useState<EngineSandboxShown[]>([]);
  const [dismissed, setDismissed] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | null>(null);

  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .engineSandbox()
        .then((answer) => {
          if (gone || answer.status !== "ok") return;
          // A core that answers nothing has no Notice to draw.
          setShown(answer.data ?? []);
        })
        .catch(() => undefined);
    read();
    const listening = listen<EngineMachine | null>(ENGINE_SANDBOX_NEEDED, (event) => {
      if (gone) return;
      // The machine that refused again stands again; another one dismissed stays dismissed.
      const again = event.payload;
      if (again) setDismissed((now) => now.filter((one) => one !== keyOf(again)));
      read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  const useChrome = () => {
    setBusy(true);
    void commands
      .useThisMachinesChrome()
      .then((answer) => {
        if (answer.status === "ok")
          setShown((now) => now.filter((one) => one.machine.kind !== "here"));
        else setWhy(answer.error);
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  };
  // One Notice for each machine whose engine cannot make its sandbox: this one's, and each
  // pool's runner's (#2990).
  const machine = (one: EngineSandboxShown) => keyOf(one.machine);
  return (
    <>
      {shown
        .filter((one) => !dismissed.includes(machine(one)))
        .map((one) => (
          <Notice
            key={machine(one)}
            cause="engine-sandbox"
            tone="trouble"
            fixes={
              one.chrome && one.machine.kind === "here"
                ? [{ label: "Use this machine's Chrome", onPress: useChrome, busy }]
                : undefined
            }
            copy={one.command ?? undefined}
            onDismiss={() => setDismissed((now) => [...now, machine(one)])}
          >
            {one.says}
            {why && one.machine.kind === "here" && ` ${why}`}
          </Notice>
        ))}
    </>
  );
}
