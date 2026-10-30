// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio as its stories draw it** (#3992): its tab, `width` by `height` px, in the
 * content panel as a view tab is drawn (`PlaneView`, `panelRoot("content")`). The panel's fill is
 * the ground every sticky part of the panel family is drawn on (the view frame's page header, the
 * panel section's top) while a pane scrolls under it (#3944). Isolated as the window's root is
 * (`App.css`, #1481), so a dialog opened over the page is drawn over its sticky header here too.
 */
import { SKILL_STUDIO_VIEW, studioHistoryOf } from "./skillStudio";
import type { ViewRef } from "./tabs";
import { SkillStudioTab } from "./SkillStudioTab";
import { Panel } from "./ui/panel/Panel";

/** The plane every studio story's core answers for. */
export const STORY_PLANE = "/home/dev/plane";

/** The history the studio of `STORY_PLANE` opens at, where a story puts its place. */
export const STORY_HISTORY = studioHistoryOf(STORY_PLANE, SKILL_STUDIO_VIEW);

export function StudioInPanel({
  width,
  height,
  narrow,
  onOpenView,
}: {
  width: number;
  height: number;
  narrow?: boolean;
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  return (
    // The panel is the tab's size, as `PlaneView` draws it, so its ground and corners are the
    // tab's edges, not the canvas's.
    <div style={{ inlineSize: width }}>
      <Panel variant="content">
        <div
          className="view-body"
          style={{ inlineSize: width, blockSize: height, display: "flex", isolation: "isolate" }}
        >
          <SkillStudioTab plane={STORY_PLANE} changed={0} narrow={narrow} onOpenView={onOpenView} />
        </div>
      </Panel>
    </div>
  );
}
