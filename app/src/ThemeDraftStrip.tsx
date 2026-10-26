// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { Palette } from "lucide-react";

import { commands, type PlaneId, type SettingsChange } from "./bindings";
import { compareWithKept, drawWhatIsInForce } from "./Extensions";
import { keepSays, revertKeySaid, useDraftHere } from "./themeDrafts";
import { pressed } from "./keys/registry";
import { Button } from "./ui/button/Button";
import { AlertDialog } from "./ui/dialog/Dialog";

/** What Keep asks once it has kept: the theme, and the level it went to. */
type Kept = { folder: string; workspace: string | null; keptTo: string };

/**
 * **The theme draft strip** (#2332, spec #2313): while a chat working in this workspace has a
 * theme draft, one line says so — "Theme draft — <name>, edited by <chat>" — with **Keep**,
 * **Revert** and **Compare**. The core draws the draft only here, only while it holds every hard
 * floor; when a save breaks one, the line says which and the last good version stays drawn.
 *
 * - **Keep** copies the draft into the real folder it is a draft of, then asks once whether the
 *   theme becomes this workspace's (or the project's) committed default or stays the person's
 *   own choice ({@link KeepAsk}). Where that folder is the project's, the strip says so before
 *   it is pressed; where Keep replaces a folder that is there, it asks first, saying when that
 *   folder changed since the draft started.
 * - **Revert** deletes the draft.
 * - **Compare**, while it is held (pointer or key), draws the theme as kept.
 * - **The revert key** (⌘⇧⌫, Ctrl+Shift+Backspace elsewhere) reverts every draft here at once,
 *   whatever the window looks like.
 * - **While the draft is drawn, a refused kept theme's Notice waits** (#2708): the strip says
 *   so, and Revert brings it back.
 */
export function ThemeDraftStrip({
  plane,
  workspace,
}: {
  plane: PlaneId;
  /** The workspace in front, `null` at the plane root. */
  workspace: string | null;
}) {
  const here = useDraftHere();
  const [said, setSaid] = useState<string>();
  const [keeping, setKeeping] = useState<Kept>();
  const [replacing, setReplacing] = useState(false);
  const holding = useRef(false);
  const draft =
    here !== null && here.plane === plane && here.workspace === workspace ? here.draft : null;

  // The revert key, while a draft is here: heard before anything else in the window, so a
  // terminal or an editor in focus does not take it.
  useEffect(() => {
    if (draft === null) return;
    const heard = (event: KeyboardEvent) => {
      // The key registry's (#2921): reserved, held back by nothing.
      if (pressed(event, ["theme.revert"]) === undefined) return;
      event.preventDefault();
      event.stopPropagation();
      void commands.themeDraftsRevertHere(plane, workspace).then((done) => {
        setSaid(done.status === "error" ? done.error : undefined);
        void drawWhatIsInForce();
      });
    };
    window.addEventListener("keydown", heard, true);
    return () => window.removeEventListener("keydown", heard, true);
  }, [draft, plane, workspace]);

  if (draft === null) {
    return keeping === undefined ? null : (
      <KeepAsk
        plane={plane}
        folder={keeping.folder}
        workspace={keeping.workspace}
        keptTo={keeping.keptTo}
        onDone={() => setKeeping(undefined)}
      />
    );
  }

  const compare = (held: boolean) => {
    if (holding.current === held) return;
    holding.current = held;
    void compareWithKept(held);
  };
  // Keep does what the strip showed: where it puts the draft, whether it replaces a folder there,
  // and whether that folder changed since the draft started, which the person agreed to before
  // this is called. The core refuses when any has changed since.
  const keep = () => {
    setReplacing(false);
    void commands
      .themeDraftKeep(
        plane,
        draft.session,
        draft.folder,
        draft.kept_to,
        draft.replaces,
        draft.changed,
      )
      .then((done) => {
        if (done.status === "error") {
          setSaid(done.error);
          return;
        }
        setSaid(done.data);
        setKeeping({ folder: draft.folder, workspace: draft.workspace, keptTo: draft.kept_to });
        void drawWhatIsInForce();
      });
  };
  const revert = () => {
    void commands.themeDraftRevert(plane, draft.session, draft.folder).then((done) => {
      setSaid(done.status === "error" ? done.error : undefined);
      void drawWhatIsInForce();
    });
  };
  const notDrawn = draft.broken[0] ?? draft.refused[0];
  return (
    <div className="read-only-line theme-draft-strip" role="region" aria-label="Theme draft">
      <Palette aria-hidden="true" />
      <div className="theme-draft-words">
        <span>
          Theme draft — {draft.folder}, edited by {draft.chat}
        </span>
        <span>{keepSays(draft)}</span>
        {notDrawn !== undefined && (
          <span role="status">
            {draft.drawn
              ? `Its last save is not drawn, so its last good version is: ${notDrawn}`
              : `Not drawn: ${notDrawn}`}
          </span>
        )}
        {draft.drawn && draft.kept_refused && <span role="status">{KEPT_REFUSED_WAITS}</span>}
        {said !== undefined && <span role="status">{said}</span>}
      </div>
      <div className="theme-draft-actions">
        {/* #190: WebKit leaves a button out of the tab sequence without `tabIndex`. */}
        <Button onPress={() => (draft.replaces ? setReplacing(true) : keep())}>Keep</Button>
        <Button
          tooltip={`Revert every theme draft here at once: ${revertKeySaid()}`}
          onPress={revert}
        >
          Revert
        </Button>
        <Button
          tooltip="Hold to see the theme as it was last kept"
          onPointerDown={() => compare(true)}
          onPointerUp={() => compare(false)}
          onPointerLeave={() => compare(false)}
          onBlur={() => compare(false)}
          onKeyDown={(event) => {
            if (event.key === " " || event.key === "Enter") {
              event.preventDefault();
              compare(true);
            }
          }}
          onKeyUp={(event) => {
            if (event.key === " " || event.key === "Enter") compare(false);
          }}
        >
          Compare
        </Button>
      </div>
      {replacing && (
        <ReplaceAsk
          folder={draft.folder}
          says={keepSays(draft)}
          onReplace={keep}
          onCancel={() => setReplacing(false)}
        />
      )}
      {keeping !== undefined && (
        <KeepAsk
          plane={plane}
          folder={keeping.folder}
          workspace={keeping.workspace}
          keptTo={keeping.keptTo}
          onDone={() => setKeeping(undefined)}
        />
      )}
    </div>
  );
}

/** What the strip says while the draft is drawn over a kept theme that is refused (#2708). */
export const KEPT_REFUSED_WAITS =
  "The theme as kept is refused, and its Notice waits while this draft is drawn: Revert brings it back.";

/**
 * **Keep asks before it replaces a theme folder that is there** (#2332): the folder it is a draft
 * of is committed for whoever reads that level, so replacing it is the person's explicit yes.
 * Escape, and the focus, are Cancel.
 */
function ReplaceAsk({
  folder,
  says,
  onReplace,
  onCancel,
}: {
  folder: string;
  says: string;
  onReplace: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog
      onClose={onCancel}
      title={`Replace ${folder}?`}
      description={
        <p className="honest">{says} The theme there now is gone once you replace it.</p>
      }
      main={{ children: "Replace", variant: "danger", onPress: onReplace }}
    />
  );
}

/** The edit that names `folder` as the theme a manifest picks. */
function picking(folder: string): SettingsChange {
  return {
    kind: "edits",
    edits: [{ path: [{ key: "theme" }, { key: "use" }], value: { kind: "text", value: folder } }],
  };
}

/**
 * **Keep asks once** (spec #2313): whether the kept theme becomes the workspace's committed
 * default (the project's, at the plane root), which the team then draws, or stays the person's
 * own choice here, on this machine. Escape answers the person's own: nothing is shared unasked.
 */
export function KeepAsk({
  plane,
  folder,
  workspace: worked,
  keptTo,
  onDone,
}: {
  plane: PlaneId;
  folder: string;
  /** Where the chat worked, `null` at the plane root. */
  workspace: string | null;
  /** Where the theme was kept to (`WindowDraft.kept_to`): `workspace` or `project`. The
   *  question is about that level. */
  keptTo: string;
  onDone: () => void;
}) {
  const [said, setSaid] = useState<string>();
  const workspace = keptTo === "project" ? null : worked;
  const where = workspace === null ? "the project" : `the workspace ${workspace}`;
  const done = () => {
    void drawWhatIsInForce();
    onDone();
  };
  const asMine = () => {
    void commands.themeChoose(plane, workspace, folder, null, keptTo).then((chose) => {
      if (chose.status === "error") setSaid(chose.error);
      else done();
    });
  };
  const asDefault = async () => {
    const wrote =
      workspace === null
        ? await commands
            .projectSettings(plane)
            .then((read) =>
              read.status === "error"
                ? read
                : commands.saveProjectSettings(
                    plane,
                    "shared",
                    read.data.shared.exists ? read.data.shared.text : null,
                    picking(folder),
                  ),
            )
        : await commands
            .workspaceSettings(plane, workspace)
            .then((read) =>
              read.status === "error"
                ? read
                : commands.saveWorkspaceSettings(
                    plane,
                    workspace,
                    read.data.exists ? read.data.text : null,
                    picking(folder),
                  ),
            );
    if (wrote.status === "error") setSaid(wrote.error);
    else if (wrote.data.kind === "refused") setSaid(wrote.data.reasons.join(" "));
    else done();
  };
  // **Placed as every dialog is** (#2910): it had no place of its own and sat wherever the
  // window drew it. The person's own choice leads and has the keyboard, as Escape is.
  return (
    <AlertDialog
      onClose={asMine}
      title={`Who draws ${folder}?`}
      description={
        <p className="honest">
          {folder} is kept. It can be your own choice in {where} on this machine, or {where}'s
          default, which is saved with the project for everyone who works in it.
        </p>
      }
      cancel={{ children: "Only for me" }}
      main={{ children: `Make it ${where}'s default`, onPress: () => void asDefault() }}
    >
      {said !== undefined && (
        <p className="honest" role="status">
          {said}
        </p>
      )}
    </AlertDialog>
  );
}
