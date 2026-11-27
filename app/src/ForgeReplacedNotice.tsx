// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ForgeReplaced, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { useWindowFocused } from "./windowFocused";

/** The family of the Notice's cause, whose dismissal is kept until the project's old lines
 *  change (`dismissals.ts`' `KEPT`). */
export const FORGE_REPLACED = "forge-sign-in-replaces";

/**
 * **A project's old forge token lines, replaced by the person's sign-in** (ADR 0128,
 * "Migration"): with the one-login train's feature on, `mint = "github-app"` and a vault forge
 * token still load and run nothing. This says so once for the lines the project has now: Dismiss
 * keeps it dismissed until they change (the core's cause key names them), and Open sign-in goes to
 * Settings › Accounts. Nothing in a project file or a vault is changed from here.
 *
 * Read from the sandbox state when the project opens, when its settings change on disk and when
 * the window comes back; `null` (always, without the feature) draws nothing.
 */
export function ForgeReplacedNotice({
  plane,
  dismissed,
  dismiss,
  settle,
  onSignIn,
}: {
  plane: PlaneId;
  /** The causes dismissed in this project, and the ways to change them (`useDismissals`). */
  dismissed: ReadonlySet<string>;
  dismiss: (cause: string) => void;
  settle: (family: string, present: readonly string[]) => void;
  /** Opens Settings › Accounts. */
  onSignIn: () => void;
}) {
  const [told, setTold] = useState<ForgeReplaced | null>(null);
  const onDisk = usePlaneChanged([plane], SETTINGS);
  const focused = useWindowFocused();
  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((answer) => {
        if (!live || answer.status !== "ok") return;
        const now = answer.data?.forge_replaced ?? null;
        setTold(now);
        // A dismissal stands while the lines do; once they change it shows again.
        settle(FORGE_REPLACED, now === null ? [] : [now.cause]);
      })
      // A project whose state cannot be read says nothing here.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused, settle]);
  if (told === null || dismissed.has(told.cause)) return null;
  return (
    <Notice
      cause={told.cause}
      label="Forge token lines replaced by your sign-in"
      fixes={[{ label: "Open sign-in", onPress: onSignIn }]}
      onDismiss={() => dismiss(told.cause)}
    >
      <p>{told.text}</p>
    </Notice>
  );
}
