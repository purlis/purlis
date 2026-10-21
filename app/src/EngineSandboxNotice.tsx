// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "./here";

import { commands, ENGINE_SANDBOX_NEEDED, type EngineSandboxShown } from "./bindings";
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
 */
export function EngineSandboxNotice() {
  const [shown, setShown] = useState<EngineSandboxShown | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | null>(null);

  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .engineSandbox()
        .then((answer) => {
          if (gone || answer.status !== "ok") return;
          setShown(answer.data);
          if (answer.data) setDismissed(false);
        })
        .catch(() => undefined);
    read();
    const listening = listen(ENGINE_SANDBOX_NEEDED, () => {
      if (!gone) read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  if (!shown || dismissed) return null;
  const useChrome = () => {
    setBusy(true);
    void commands
      .useThisMachinesChrome()
      .then((answer) => {
        if (answer.status === "ok") setShown(null);
        else setWhy(answer.error);
      })
      .catch(() => undefined)
      .finally(() => setBusy(false));
  };
  return (
    <Notice
      cause="engine-sandbox"
      tone="trouble"
      fixes={
        shown.chrome
          ? [{ label: "Use this machine's Chrome", onPress: useChrome, busy }]
          : undefined
      }
      copy={shown.command ?? undefined}
      onDismiss={() => setDismissed(true)}
    >
      {shown.says}
      {why && ` ${why}`}
    </Notice>
  );
}
