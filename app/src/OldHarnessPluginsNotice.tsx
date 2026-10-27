// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type OldHarnessPlugins, type PlaneId } from "./bindings";
import { asksMoved } from "./asks";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { askOpenProposal } from "./proposedChangeOpen";
import { inertly } from "./sandboxBlocks";

/** Whether this page may answer the Notice: the window may, a link may not. */
export const mayAnswerOldPlugins = (): boolean =>
  inTheWindow(commands, "removeOldHarnessPlugins");

/**
 * **The old `[harness_plugins]` keys, said once** (#3467): purlis no longer turns other
 * harnesses' plugins on or off per project, so a project that still holds the keys starts as any
 * other, with them ignored, and is told so once, in purlis's words (`said`). **Remove** holds a
 * proposed change for each of the project's own files that takes the keys out, and opens the
 * first one's tab, where the person sees it whole and presses Apply: nothing is written before.
 * **Dismiss** keeps it away. Either answer is kept on this machine, so it is said once.
 */
export function OldHarnessPluginsNotice({
  plane,
  canAnswer = mayAnswerOldPlugins(),
}: {
  plane: PlaneId;
  /** Whether this page may answer: the window may, a link may not. */
  canAnswer?: boolean;
}) {
  const [old, setOld] = useState<OldHarnessPlugins | null>(null);
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [hidden, setHidden] = useState(false);
  const onDisk = usePlaneChanged([plane], SETTINGS);

  useEffect(() => {
    let live = true;
    void commands
      .oldHarnessPlugins(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setOld(answer.data);
      })
      // A state that cannot be read says nothing: the keys are ignored either way.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk]);

  if (old === null || hidden) return null;

  const answered = (sent: Promise<{ status: "ok" } | { status: "error"; error: string }>) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void sent
      .then((done) => (done.status === "ok" ? setHidden(true) : setSaid(done.error)))
      .catch((err: unknown) => setSaid(`purlis could not keep your answer: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const remove = () =>
    answered(
      commands.removeOldHarnessPlugins(plane).then((done) => {
        if (done.status !== "ok") return done;
        asksMoved(plane);
        const [first] = done.data;
        const file = old.files.find((one) => !one.startsWith("workspaces/"));
        if (first !== undefined && file !== undefined) askOpenProposal(plane, first, file);
        return { status: "ok" as const };
      }),
    );
  const dismiss = () => answered(commands.dismissOldHarnessPlugins(plane));
  const body = (
    <p>
      {inertly(old.said)}
      {!canAnswer && " Answer it from purlis's own window on this machine."}
      {said !== undefined && ` ${said}`}
    </p>
  );
  if (!canAnswer || !old.removable)
    return (
      <Notice
        cause="old-harness-plugins"
        label="Old harness plugin settings"
        onDismiss={canAnswer ? dismiss : () => setHidden(true)}
      >
        {body}
      </Notice>
    );
  return (
    <Notice
      cause="old-harness-plugins"
      label="Old harness plugin settings"
      fixes={[{ label: "Remove", onPress: remove, busy }]}
      onDismiss={dismiss}
      dismissSays="Leaves the keys where they are, ignored, and says this no more."
    >
      {body}
    </Notice>
  );
}
