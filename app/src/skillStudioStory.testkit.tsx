// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio as its stories draw it** (#3992): its tab, `width` by `height` px, in the
 * content panel as a view tab is drawn (`PlaneView`, `panelRoot("content")`). The panel's fill is
 * the ground every sticky part of the panel family is drawn on (the view frame's page header, the
 * panel section's top) while a pane scrolls under it (#3944). Isolated as the window's root is
 * (`App.css`, #1481), so a dialog opened over the page is drawn over its sticky header here too.
 */
import type { ViewRef } from "./tabs";
import { SkillStudioTab } from "./SkillStudioTab";
import { Panel } from "./ui/panel/Panel";

/** The plane every studio story's core answers for. */
export const STORY_PLANE = "/home/dev/plane";

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
    <Panel variant="content">
      <div
        className="view-body"
        style={{ inlineSize: width, blockSize: height, display: "flex", isolation: "isolate" }}
      >
        <SkillStudioTab plane={STORY_PLANE} changed={0} narrow={narrow} onOpenView={onOpenView} />
      </div>
    </Panel>
  );
}
