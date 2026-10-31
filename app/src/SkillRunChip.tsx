// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { Sparkles } from "lucide-react";
import { counted } from "./dispatches";
import { tokensShort, useOpenRunOf } from "./skillRuns";
import { useSkillStudioOn } from "./skillStudio";
import { Button } from "./ui/button/Button";

/**
 * **The skill run open in a tab's chat** (SK-3, #2766): the skill's name, the tool calls made
 * inside it so far and, where the chat's harness says its usage, the tokens it has spent since
 * (SK-26, #3694, decision 41), from the moment the chat uses the skill until its turn ends.
 * Where the harness says no usage it says no tokens, never a zero. Pressed,
 * it opens the Skill studio at that skill's Runs, with the run marked (#3682). The skill is the one the core counts the run of, by its
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
  /** Opens the skill's Runs in the studio, with the run marked, for the chats of `session`'s
   *  place, or its persona's for a persona's own skill (`level`, #3694). */
  onOpen: (skill: string, run: string, session: number, level: string | null) => void;
}) {
  const on = useSkillStudioOn();
  const run = useOpenRunOf(session);
  if (!on || run === undefined) return null;
  const calls = counted(run.tool_calls, "tool call", "tool calls");
  const tokens = run.tokens === null ? undefined : `${tokensShort(run.tokens)} tokens`;
  const sofar = tokens === undefined ? calls : `${calls} and ${tokens}`;
  return (
    <span className="tab-tasks tab-skill-run">
      {/* On the chip's pill, as a tab's task counts are (`TabChip`, #3264). */}
      <span className="tab-tasks-pill">
        <Button
          variant="quiet"
          size="sm"
          className="tab-tasks-counts"
          data-testid="tab-skill-run"
          tooltip={`Using the skill ${run.name}: ${sofar} so far. Open its runs.`}
          aria-label={`Using the skill ${run.name}, ${sofar} so far. Open its runs`}
          onPress={() => session !== undefined && onOpen(run.name, run.id, session, run.level)}
        >
          <span className="count">
            <Sparkles aria-hidden="true" />
            <span className="skill-run-name">{run.name}</span>
          </span>
          <span className="count">{run.tool_calls}</span>
          {tokens !== undefined && (
            <span className="count skill-run-tokens" data-testid="tab-skill-run-tokens">
              {tokens}
            </span>
          )}
        </Button>
      </span>
    </span>
  );
});
