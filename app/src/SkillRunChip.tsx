// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { Sparkles } from "lucide-react";
import { counted } from "./dispatches";
import { useOpenRunOf } from "./skillRuns";
import { useSkillStudioOn } from "./skillStudio";
import { Button } from "./ui/button/Button";

/**
 * **The skill run open in a tab's chat** (SK-3, #2766): the skill's name and the tool calls
 * made inside it so far, from the moment the chat uses the skill until its turn ends. Pressed,
 * it opens the skill's tab on that run. The skill is the one the core counts the run of, by its
 * own name (`secrets` for opencode's `purlis-secrets`, #3385): the window reads no name itself.
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
  const calls = counted(run.tool_calls, "tool call", "tool calls");
  return (
    <span className="tab-tasks tab-skill-run">
      {/* On the chip's pill, as a tab's task counts are (`TabChip`, #3264). */}
      <span className="tab-tasks-pill">
        <Button
          variant="quiet"
          size="sm"
          className="tab-tasks-counts"
          data-testid="tab-skill-run"
          tooltip={`Using the skill ${run.name}: ${calls} so far. Open its runs.`}
          aria-label={`Using the skill ${run.name}, ${calls} so far. Open its runs`}
          onPress={() => onOpen(run.name, run.id)}
        >
          <span className="count">
            <Sparkles aria-hidden="true" />
            <span className="skill-run-name">{run.name}</span>
          </span>
          <span className="count">{run.tool_calls}</span>
        </Button>
      </span>
    </span>
  );
});
