// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { Sparkles } from "lucide-react";
import { useOpenRunOf } from "./skillRuns";
import { useSkillStudioOn } from "./skillStudio";

/** How a run's tool calls are said: "1 tool call", "3 tool calls". */
export function toolCallsSaid(count: number): string {
  return count === 1 ? "1 tool call" : `${count} tool calls`;
}

/**
 * **The skill run open in a tab's chat** (SK-3, #2766): the skill's name and the tool calls
 * made inside it so far, from the moment the chat uses the skill until its turn ends. Pressed,
 * it opens the skill's tab on that run.
 *
 * Beside the tab's button, not inside it: the tab is a button, and a button holds no button.
 * Nothing while no run is open, and nothing while the Skill studio is not on.
 */
export const SkillRunChip = memo(function SkillRunChip({
  session,
  onOpen,
}: {
  session: number | undefined;
  onOpen: (skill: string, run: string) => void;
}) {
  const on = useSkillStudioOn();
  const run = useOpenRunOf(session);
  if (!on || run === undefined) return null;
  const calls = toolCallsSaid(run.tool_calls);
  return (
    <span className="tab-tasks tab-skill-run">
      <button
        type="button"
        className="tab-tasks-counts"
        data-testid="tab-skill-run"
        title={`Using the skill ${run.skill}: ${calls} so far. Open its runs.`}
        aria-label={`Using the skill ${run.skill}, ${calls} so far. Open its runs`}
        onClick={() => onOpen(run.skill, run.id)}
      >
        <span className="count">
          <Sparkles aria-hidden="true" />
          <span className="skill-run-name">{run.skill}</span>
        </span>
        <span className="count">{run.tool_calls}</span>
      </button>
    </span>
  );
});
