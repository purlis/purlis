// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands } from "./bindings";
import { Notice } from "./Notice";

/**
 * **Where this project's browsers were when purlis last quit** (#2022, ruled 2026-10-18): quit
 * ends every browser, and this start offers each address once, as a button, never loaded by
 * itself. Until a browser the person opens can go to an address (#2176), a button copies its
 * address; Dismiss puts them away for good.
 */
export function BrowsersAtQuitNotice({ plane }: { plane: string }) {
  const [addresses, setAddresses] = useState<readonly string[]>([]);
  useEffect(() => {
    let gone = false;
    void commands
      .browsersAtQuit(plane)
      .then((said) => {
        // A core that answered nothing (an older one, or none) offers nothing.
        if (!gone && said.status === "ok" && Array.isArray(said.data)) setAddresses(said.data);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane]);
  const [first, ...rest] = addresses;
  if (first === undefined) return null;
  const copy = (address: string) => ({
    label: `Copy ${hostOf(address)}`,
    onPress: () => void navigator.clipboard?.writeText(address).catch(() => undefined),
  });
  return (
    <Notice
      cause="browsers-at-quit"
      fixes={[copy(first), ...rest.map(copy)]}
      onDismiss={() => {
        setAddresses([]);
        void commands.putAwayBrowsersAtQuit(plane).catch(() => undefined);
      }}
      dismissSays="They are not offered again."
    >
      When purlis last quit, this project's browsers were at {addresses.join(", ")}. Nothing was
      loaded again.
    </Notice>
  );
}

/** What a button names an address by: its host, or the address where it has none. */
function hostOf(address: string): string {
  try {
    return new URL(address).host || address;
  } catch {
    return address;
  }
}
