// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { Users } from "lucide-react";
import {
  commands,
  type ArtifactPage,
  type ArtifactPinAsk,
  type ArtifactPinShown,
  type PlaneId,
} from "./bindings";
import { PERSONA_ICONS } from "./PersonaMark";
import { projectName } from "./projectName";
import { Field, SettingRow } from "./settings/components";
import { OptionCard, OptionCards } from "./ui/choice/Radio";
import { AlertDialog, Dialog } from "./ui/dialog/Dialog";

/**
 * **Pin to workspace** (#5953, ADR 0120 §1): the team's pin, never a tab's private Pin. It reads
 * differently (it names the workspace and asks first) and looks different (its mark is people,
 * not a pin), so one is never taken for the other (spec story 11).
 *
 * - **It says who will see it** before anything is written: "Visible to everyone in
 *   \<project\>" in a LIVE workspace, and that it stays on this machine in a LOCAL one.
 * - **A name, suggested from the page's title, and an icon from purlis's set**, never the
 *   page's own image.
 * - **A page outside the workspace's `artifacts/`** is copied into a folder of its own there,
 *   and the dialog names the folder.
 *
 * Nothing is committed here: the pins file is written, and the project's next save shares it.
 */
export function PinToWorkspace({
  project,
  workspace,
  ask,
  pinning,
  trouble,
  onPin,
  onCancel,
}: {
  /** The project's name, as the window shows it. */
  project: string;
  workspace: string;
  ask: ArtifactPinAsk;
  /** Whether purlis is pinning it right now, so the answer cannot be given twice. */
  pinning: boolean;
  /** Why the last attempt pinned nothing: the core's sentence, unchanged. */
  trouble?: string;
  onPin: (name: string, icon: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(ask.name);
  const [icon, setIcon] = useState(ask.icon);
  const ready = name.trim() !== "" && !pinning;
  return (
    <Dialog
      onClose={onCancel}
      title="Pin to workspace"
      titleId="pin-to-workspace"
      subtitle={workspace}
      data-testid="pin-to-workspace"
      description={
        <span className="pin-reach" data-reach={ask.live ? "everyone" : "this-machine"}>
          <Users aria-hidden="true" className="node-icon" />
          {reachOf(project, workspace, ask.live)}
        </span>
      }
      summary={
        ask.copied
          ? `The page is copied to artifacts/${ask.folder}/ in ${workspace}.`
          : ask.pinned
            ? "It is pinned already: this changes its name and icon."
            : undefined
      }
      main={{
        children: pinning ? "Pinning…" : "Pin to workspace",
        reserve: ["Pin to workspace", "Pinning…"],
        isDisabled: !ready,
        onPress: () => {
          if (ready) onPin(name.trim(), icon);
        },
      }}
    >
      <SettingRow
        label="Name"
        help="What its icon is called on the activity bar."
        control={(ids) => <Field ids={ids} kind="text" value={name} onChange={setName} />}
      />
      <OptionCards
        variant="tile"
        aria-label="Icon"
        orientation="horizontal"
        value={icon}
        onChange={setIcon}
      >
        {Object.entries(PERSONA_ICONS).map(([one, Icon]) => (
          <OptionCard
            key={one}
            variant="tile"
            value={one}
            aria-label={one}
            tooltip={one}
            icon={<Icon aria-hidden="true" />}
          />
        ))}
      </OptionCards>
      {/* Verbatim, beside the answers: the person is still answering. */}
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}

/** Who sees a pin of workspace `workspace`: everyone in the project, or this machine alone. */
export function reachOf(project: string, workspace: string, live: boolean): string {
  return live
    ? `Visible to everyone in ${project} once the project is saved.`
    : `${workspace} is a LOCAL workspace, so this artifact stays on this machine.`;
}

/**
 * **Pin to workspace, for a page the window names** (`actions.ts`'s `pinToWorkspace`): asks the
 * core what to offer, draws {@link PinToWorkspace}, and pins on the person's answer.
 */
export function PinPage({
  plane,
  workspace,
  page,
  onClose,
  onPinned,
}: {
  plane: PlaneId;
  workspace: string;
  page: ArtifactPage;
  onClose: () => void;
  onPinned?: (pin: ArtifactPinShown) => void;
}) {
  const [ask, setAsk] = useState<{ ask?: ArtifactPinAsk; failed?: string }>({});
  const [pinning, setPinning] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  useEffect(() => {
    let gone = false;
    commands.artifactPinAsk(plane, workspace, page).then(
      (said) => {
        if (gone) return;
        setAsk(said.status === "ok" ? { ask: said.data } : { failed: said.error });
      },
      (why: unknown) => {
        if (!gone) setAsk({ failed: String(why) });
      },
    );
    return () => {
      gone = true;
    };
    // The page is named by its fields; a new object for the same page asks nothing new.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plane, workspace, page.workspace, page.repo, page.piece, page.path]);
  if (ask.failed !== undefined)
    return (
      <AlertDialog
        onClose={onClose}
        title="This page cannot be pinned"
        description={ask.failed}
        cancel={{ children: "Close" }}
        data-testid="pin-to-workspace-refused"
      />
    );
  if (ask.ask === undefined) return null;
  const offered = ask.ask;
  return (
    <PinToWorkspace
      project={projectName(plane)}
      workspace={workspace}
      ask={offered}
      pinning={pinning}
      trouble={trouble}
      onCancel={onClose}
      onPin={(name, icon) => {
        setPinning(true);
        setTrouble(undefined);
        commands
          .artifactPin(plane, workspace, page, { folder: offered.folder, name, icon })
          .then(
            (said) => {
              if (said.status === "error") {
                setTrouble(said.error);
                return;
              }
              onPinned?.(said.data);
              onClose();
            },
            (why: unknown) => setTrouble(String(why)),
          )
          .finally(() => setPinning(false));
      }}
    />
  );
}

/**
 * **Unpin from workspace** (#5953): asks first, for it takes the icon off everyone's bar once the
 * project is saved. Its folder stays, so pinning it again brings it back.
 */
export function UnpinArtifact({
  project,
  workspace,
  pin,
  live,
  unpinning,
  trouble,
  onUnpin,
  onCancel,
}: {
  project: string;
  workspace: string;
  pin: ArtifactPinShown;
  /** Whether the workspace is LIVE: the pin leaves everyone's bar, not only this machine's. */
  live: boolean;
  unpinning: boolean;
  trouble?: string;
  onUnpin: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog
      onClose={onCancel}
      title={`Unpin ${pin.name} from ${workspace}`}
      data-testid="unpin-artifact"
      description={
        live
          ? `Its icon leaves the activity bar for everyone in ${project} once the project is saved. Its files stay in artifacts/${pin.folder}/.`
          : `Its icon leaves the activity bar. Its files stay in artifacts/${pin.folder}/.`
      }
      main={{
        children: unpinning ? "Unpinning…" : "Unpin",
        reserve: ["Unpin", "Unpinning…"],
        isDisabled: unpinning,
        onPress: onUnpin,
      }}
    >
      {trouble && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </AlertDialog>
  );
}
