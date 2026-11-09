// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import { ChartLine } from "lucide-react";
import { commands, type ArtifactPinShown, type PlaneId } from "./bindings";
import { PERSONA_ICONS } from "./PersonaMark";
import { useViewChanged } from "./planeChanged";
import { useTabStop } from "./roving";
import { ActivityGroup, ActivityItem } from "./ui/activity/ActivityItem";
import { Separator } from "./ui/separator/Separator";

/** The most icons the group draws (ADR 0120 §2); more go to the bar's show-more (#5953). */
export const MOST_DRAWN = 8;

/**
 * **A workspace's pinned artifacts on the left activity bar** (ADR 0120 §2, #5952): a group of
 * their own, after a divider, below purlis's own icons, which never move for it. One icon per
 * pin, in the pins file's order, its mark one of purlis's icons and its name the pin's.
 *
 * **A press opens the artifact's tab, or brings it forward** (`openView`): the group is a
 * toolbar of buttons, not a tab list, since what it opens is a tab of the centre and not a view
 * of the side. The icon of the artifact in front is drawn chosen, and says so (`aria-current`).
 * The arrows walk it with one Tab stop, as they walk the bar above.
 *
 * Nothing is drawn while the workspace has no pins, nor outside every workspace.
 */
export function ArtifactGroup({
  plane,
  workspace,
  pins: given,
  current,
  onOpen,
}: {
  plane: PlaneId;
  /** The workspace in front, or nothing outside every workspace. */
  workspace: string | undefined;
  /** The pins as given, in place of asking the core: a story's, a test's. */
  pins?: readonly ArtifactPinShown[];
  /** The folder of the artifact whose tab is in front, if one is. */
  current?: string;
  onOpen: (pin: ArtifactPinShown) => void;
}) {
  const asked = usePins(plane, given === undefined ? workspace : undefined);
  const pins = (given ?? asked).slice(0, MOST_DRAWN);
  const stop = useTabStop(
    current,
    pins.map((pin) => pin.folder),
  );
  if (workspace === undefined || pins.length === 0) return null;
  return (
    <>
      <Separator decorative className="artifact-divider" />
      <RovingFocusGroup.Root asChild orientation="vertical" loop={false} {...stop}>
        <ActivityGroup
          role="toolbar"
          aria-orientation="vertical"
          aria-label="Artifacts"
          className="activity-bar artifact-group"
        >
          {pins.map((pin) => {
            const Glyph = Object.prototype.hasOwnProperty.call(PERSONA_ICONS, pin.icon)
              ? PERSONA_ICONS[pin.icon]
              : ChartLine;
            const chosen = pin.folder === current;
            const unopenable = pin.missing ?? pin.too_large;
            return (
              <RovingFocusGroup.Item key={pin.id} asChild tabStopId={pin.folder} active={chosen}>
                <ActivityItem
                  variant="bar"
                  selected={chosen}
                  tip={unopenable === null ? `Open ${pin.name}` : unopenable}
                  aria-label={pin.name}
                  aria-current={chosen ? "page" : undefined}
                  data-artifact={pin.folder}
                  data-missing={unopenable === null ? undefined : true}
                  onClick={() => onOpen(pin)}
                  icon={<Glyph aria-hidden="true" />}
                />
              </RovingFocusGroup.Item>
            );
          })}
        </ActivityGroup>
      </RovingFocusGroup.Root>
    </>
  );
}

/** The workspace's pins as the core reads them, read again when the workspace changes on disk. */
function usePins(plane: PlaneId, workspace: string | undefined): readonly ArtifactPinShown[] {
  // Kept with the workspace they are of, so another workspace's are never drawn for this one.
  const [read, setRead] = useState<{ of: string; pins: readonly ArtifactPinShown[] }>({
    of: "",
    pins: [],
  });
  const changed = useViewChanged(
    [plane],
    workspace === undefined
      ? undefined
      : (change) => change.workspace === null || change.workspace === workspace,
  );
  useEffect(() => {
    if (workspace === undefined) return;
    let gone = false;
    const of = `${plane}\u0000${workspace}`;
    void commands.artifactPins(plane, workspace).then(
      (said) => {
        if (!gone) setRead({ of, pins: said.status === "ok" ? said.data : [] });
      },
      () => {
        if (!gone) setRead({ of, pins: [] });
      },
    );
    return () => {
      gone = true;
    };
  }, [plane, workspace, changed]);
  return workspace !== undefined && read.of === `${plane}\u0000${workspace}` ? read.pins : NONE;
}

const NONE: readonly ArtifactPinShown[] = [];
