// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState, type CSSProperties } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import { closestCenter, DndContext } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { ChartLine, Ellipsis } from "lucide-react";
import { commands, type ArtifactPinShown, type PlaneId } from "./bindings";
import {
  ARTIFACT_PINS_MOVED,
  MOST_DRAWN,
  drawnAndMore,
  freshness,
  movedBy,
  pinOrderAfterDrop,
  type ArtifactPinsMoved,
} from "./artifacts";
import { ArtifactBadge, badgeDescribes } from "./ArtifactBadge";
import { listen } from "./here";
import { PERSONA_ICONS } from "./PersonaMark";
import { UnpinArtifact } from "./PinToWorkspace";
import { useViewChanged } from "./planeChanged";
import { projectName } from "./projectName";
import { useTabStop } from "./roving";
import {
  DOWN_THE_RAIL,
  SortableTab,
  keepsTheFocus,
  stripAccessibility,
  useStripSensors,
} from "./sortable";
import { ActivityGroup, ActivityItem } from "./ui/activity/ActivityItem";
import { ContextMenu } from "./ui/menu/ContextMenu";
import { Menu, MenuItem, MenuTrigger } from "./ui/menu/Menu";
import { Separator } from "./ui/separator/Separator";

export { MOST_DRAWN };

/** The tab stop of the group's show-more. */
const MORE = "artifacts-more";

/**
 * **A workspace's pinned artifacts on the left activity bar** (ADR 0122 §2, #5952): a group of
 * their own, after a divider, below purlis's own icons, which never move for it. One icon per
 * pin, in the pins file's order, its mark one of purlis's icons and its name the pin's.
 *
 * **A press opens the artifact's tab, or brings it forward** (`openView`): the group is a
 * toolbar of buttons, not a tab list, since what it opens is a tab of the centre and not a view
 * of the side. The icon of the artifact in front is drawn chosen, and says so (`aria-current`).
 * The arrows walk it with one Tab stop, as they walk the bar above.
 *
 * **Managed from the bar** (#5953):
 *
 * - **At most eight are drawn**; every pin past them, which a merge can bring, is in the
 *   group's show-more, never dropped.
 * - **Its order is the team's**: dragged (Shift+Space lifts an icon from the keyboard, as on
 *   the rail), or Move up and Move down on an icon's menu, since a drag is never the only way.
 *   Either writes the whole order to the pins file, which the project's next save shares.
 * - **Unpin from workspace…** on its menu asks first, then takes it off the pins file; its
 *   folder stays.
 *
 * **Each icon says when its data was last updated** (#5955), in its tooltip, and is dimmed once
 * that is stale (`purlis_core::artifact::fresh`), at the moment it turns, with nothing else
 * changing. The pins are read again on any change to the workspace's artifacts.
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
  const pins = given ?? asked;
  const { drawn, more } = drawnAndMore(pins);
  const now = useNowAt(drawn.map((pin) => pin.fresh?.stale_at ?? null));
  const ids = useMemo(() => pins.map((pin) => pin.id), [pins]);
  const [unpinning, setUnpinning] = useState<ArtifactPinShown>();
  const [trouble, setTrouble] = useState<string>();
  const sensors = useStripSensors();
  const words = useMemo(
    () => stripAccessibility("artifact", (id) => pins.find((pin) => pin.id === id)?.name ?? ""),
    [pins],
  );
  const stop = useTabStop(current, [
    ...drawn.map((pin) => pin.folder),
    ...(more.length > 0 ? [MORE] : []),
  ]);
  if (workspace === undefined || pins.length === 0) return null;
  const reorder = (order: readonly string[] | undefined) => {
    if (order === undefined) return;
    void commands.artifactReorder(plane, workspace, [...order]).catch(() => {});
  };
  return (
    <>
      <Separator decorative className="artifact-divider" />
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={DOWN_THE_RAIL}
        accessibility={words}
        onDragEnd={({ active, over }) => {
          if (over) reorder(pinOrderAfterDrop(ids, String(active.id), String(over.id)));
        }}
      >
        <SortableContext items={drawn.map((pin) => pin.id)} strategy={verticalListSortingStrategy}>
          <RovingFocusGroup.Root asChild orientation="vertical" loop={false} {...stop}>
            <ActivityGroup
              role="toolbar"
              aria-orientation="vertical"
              aria-label="Artifacts"
              className="activity-bar artifact-group"
            >
              {drawn.map((pin) => {
                const chosen = pin.folder === current;
                const unopenable = pin.missing ?? pin.too_large;
                const fresh = freshness(pin.fresh, now);
                return (
                  <SortableTab key={pin.id} id={pin.id}>
                    {({ sortable, style }) => (
                      <span
                        ref={sortable.setNodeRef}
                        style={style as CSSProperties}
                        className="artifact-slot"
                      >
                        <ContextMenu
                          menu={() => (
                            <PinMenu
                              pin={pin}
                              ids={ids}
                              onOpen={() => onOpen(pin)}
                              onMove={(by) => reorder(movedBy(ids, pin.id, by))}
                              onUnpin={() => {
                                setTrouble(undefined);
                                setUnpinning(pin);
                              }}
                            />
                          )}
                        >
                          <RovingFocusGroup.Item asChild tabStopId={pin.folder} active={chosen}>
                            <ActivityItem
                              variant="bar"
                              selected={chosen}
                              dimmed={fresh?.stale}
                              tip={
                                unopenable ??
                                (fresh === undefined
                                  ? `Open ${pin.name}`
                                  : `Open ${pin.name}. ${fresh.said}`)
                              }
                              aria-label={pin.name}
                              aria-current={chosen ? "page" : undefined}
                              aria-describedby={badgeDescribes(
                                pin,
                                sortable.attributes["aria-describedby"],
                              )}
                              data-artifact={pin.folder}
                              data-missing={unopenable === null ? undefined : true}
                              data-dragging={sortable.isDragging || undefined}
                              {...sortable.listeners}
                              onKeyDown={(event) => {
                                // An icon that is up is being carried: its keys are the drag's.
                                keepsTheFocus(event, sortable.isDragging);
                                sortable.listeners?.onKeyDown?.(event);
                              }}
                              onClick={() => onOpen(pin)}
                              icon={glyphOf(pin)}
                              count={<ArtifactBadge pin={pin} stale={fresh?.stale ?? false} />}
                            />
                          </RovingFocusGroup.Item>
                        </ContextMenu>
                      </span>
                    )}
                  </SortableTab>
                );
              })}
              {more.length > 0 && (
                <MenuTrigger>
                  <RovingFocusGroup.Item asChild tabStopId={MORE} active={false}>
                    <ActivityItem
                      variant="bar"
                      selected={false}
                      tip="The artifacts past the first eight"
                      aria-label={`${more.length} more artifacts`}
                      data-artifact-more=""
                      icon={<Ellipsis aria-hidden="true" />}
                    />
                  </RovingFocusGroup.Item>
                  <Menu
                    aria-label="More artifacts"
                    onAction={(key) => {
                      const pin = more.find((one) => one.id === key);
                      if (pin !== undefined) onOpen(pin);
                    }}
                  >
                    {more.map((pin) => (
                      <MenuItem key={pin.id} id={pin.id} textValue={pin.name}>
                        {pin.name}
                      </MenuItem>
                    ))}
                  </Menu>
                </MenuTrigger>
              )}
            </ActivityGroup>
          </RovingFocusGroup.Root>
        </SortableContext>
      </DndContext>
      {unpinning !== undefined && (
        <UnpinArtifact
          project={projectName(plane)}
          workspace={workspace}
          pin={unpinning}
          live={unpinning.live}
          unpinning={false}
          trouble={trouble}
          onCancel={() => setUnpinning(undefined)}
          onUnpin={() => {
            void commands.artifactUnpin(plane, workspace, unpinning.folder).then(
              (said) => {
                if (said.status === "error") setTrouble(said.error);
                else setUnpinning(undefined);
              },
              (why: unknown) => setTrouble(String(why)),
            );
          }}
        />
      )}
    </>
  );
}

/** A pin's mark: its icon from purlis's set, or the chart where it names none purlis has. */
function glyphOf(pin: ArtifactPinShown) {
  const Glyph = Object.prototype.hasOwnProperty.call(PERSONA_ICONS, pin.icon)
    ? PERSONA_ICONS[pin.icon]
    : ChartLine;
  return <Glyph aria-hidden="true" />;
}

/** **An artifact icon's menu** (#5953): open it, move it a place, or unpin it, which asks. */
function PinMenu({
  pin,
  ids,
  onOpen,
  onMove,
  onUnpin,
}: {
  pin: ArtifactPinShown;
  ids: readonly string[];
  onOpen: () => void;
  onMove: (by: -1 | 1) => void;
  onUnpin: () => void;
}) {
  const at = ids.indexOf(pin.id);
  return (
    <Menu
      aria-label={pin.name}
      disabledKeys={[
        ...(at <= 0 ? ["up"] : []),
        ...(at < 0 || at === ids.length - 1 ? ["down"] : []),
      ]}
      onAction={(key) => {
        if (key === "open") onOpen();
        else if (key === "up") onMove(-1);
        else if (key === "down") onMove(1);
        else if (key === "unpin") onUnpin();
      }}
    >
      <MenuItem id="open" textValue="Open">
        Open
      </MenuItem>
      <MenuItem id="up" textValue="Move up">
        Move up
      </MenuItem>
      <MenuItem id="down" textValue="Move down">
        Move down
      </MenuItem>
      <MenuItem id="unpin" textValue="Unpin from workspace…">
        Unpin from workspace…
      </MenuItem>
    </Menu>
  );
}

/**
 * The workspace's pins as the core reads them, read again when the workspace changes on disk,
 * and at once when a window changed them (`artifact-pins-moved`).
 */
function usePins(plane: PlaneId, workspace: string | undefined): readonly ArtifactPinShown[] {
  // Kept with the workspace they are of, so another workspace's are never drawn for this one.
  const [read, setRead] = useState<{ of: string; pins: readonly ArtifactPinShown[] }>({
    of: "",
    pins: [],
  });
  const [moved, setMoved] = useState(0);
  const changed = useViewChanged(
    [plane],
    workspace === undefined
      ? undefined
      : (change) => change.workspace === null || change.workspace === workspace,
  );
  useEffect(() => {
    if (workspace === undefined) return;
    let gone = false;
    let stop: (() => void) | undefined;
    listen<ArtifactPinsMoved>(ARTIFACT_PINS_MOVED, (event) => {
      if (event.payload.plane === plane && event.payload.workspace === workspace)
        setMoved((n) => n + 1);
    }).then(
      (unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      },
      () => {},
    );
    return () => {
      gone = true;
      try {
        stop?.();
      } catch {
        // The window's event bridge went first: nothing is listening any more.
      }
    };
  }, [plane, workspace]);
  useEffect(() => {
    if (workspace === undefined) return;
    let gone = false;
    const of = `${plane}\u0000${workspace}`;
    void commands.artifactPins(plane, workspace).then(
      (said) => {
        // Anything but a list is no pins: a window test that answers every command it does
        // not name with nothing draws no group, rather than failing in it.
        if (!gone)
          setRead({ of, pins: said.status === "ok" && Array.isArray(said.data) ? said.data : [] });
      },
      () => {
        if (!gone) setRead({ of, pins: [] });
      },
    );
    return () => {
      gone = true;
    };
  }, [plane, workspace, changed, moved]);
  return workspace !== undefined && read.of === `${plane}\u0000${workspace}` ? read.pins : NONE;
}

const NONE: readonly ArtifactPinShown[] = [];

/**
 * **Now, drawn again at each of `times` still to come** (milliseconds since 1970): an icon is
 * dimmed the moment its data turns stale, not at the next change that happens to draw the bar.
 */
function useNowAt(times: readonly (number | null)[]): number {
  const [now, setNow] = useState(() => Date.now());
  const next = Math.min(...times.filter((at): at is number => at !== null && at > now));
  useEffect(() => {
    if (!Number.isFinite(next)) return;
    // A timer's delay is held in 32 bits: a time further off is waited for in steps.
    const wait = Math.min(next - Date.now(), 2 ** 31 - 1);
    const timer = setTimeout(() => setNow(Date.now()), Math.max(0, wait));
    return () => clearTimeout(timer);
  }, [next]);
  return now;
}
