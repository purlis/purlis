// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo } from "react";
import { Sparkles } from "lucide-react";
import { counted } from "./dispatches";
import { tokensShort, useOpenRunOf } from "./skillRuns";
import { Button } from "./ui/button/Button";
import type { OpenRun } from "./bindings";

/**
 * **The skill run open in a chat, on its pane head** (SK-3, #2766; off its tab since #5524): the skill's name, the tool calls made
 * inside it so far and, where the chat's harness says its usage, the tokens it has spent since
 * (SK-26, #3694, decision 41), from the moment the chat uses the skill until its turn ends.
 * Where the harness says no usage it says no tokens, never a zero. A run of another workspace's
 * copy its persona names (`workspaces/<ws>:<name>`) says that copy, "of workspace ops", in its
 * name and its tooltip (#6083). Pressed,
 * it opens the Skill studio at that skill's Runs, with the run marked (#3682). The skill is the one the core counts the run of, by its
 * own name (`secrets` for opencode's `purlis-secrets`, #3385): the window reads no name itself.
 *
 * Nothing while no run is open.
 */
export const SkillRunChip = memo(function SkillRunChip({
  session,
  onOpen,
}: {
  session: number | undefined;
  /** Opens the skill's Runs in the studio, with run `run` marked, for the chats of `session`'s
   *  place, or its persona's for a persona's own skill (its `level`, #3694), or the named copy's
   *  workspace's (its `copy_workspace`, #6083). */
  onOpen: (run: OpenRun, session: number) => void;
}) {
  const run = useOpenRunOf(session);
  if (run === undefined) return null;
  const calls = counted(run.tool_calls, "tool call", "tool calls");
  const tokens = run.tokens === null ? undefined : `${tokensShort(run.tokens)} tokens`;
  const sofar = tokens === undefined ? calls : `${calls} and ${tokens}`;
  const copy =
    run.copy_workspace === null || run.level === null
      ? undefined
      : `of ${run.level} ${run.copy_workspace}`;
  const named = copy === undefined ? run.name : `${run.name} ${copy}`;
  return (
    <span className="pane-skill-run">
      {/* On a sunken pill, as a tab's task counts were before they moved to its pane's head
          (#3264, #5341). */}
      <span className="skill-run-pill">
        <Button
          variant="quiet"
          size="sm"
          className="skill-run-counts"
          data-testid="pane-skill-run"
          tooltip={`Using the skill ${named}: ${sofar} so far. Open its runs.`}
          onPress={() => session !== undefined && onOpen(run, session)}
        >
          {/* **Named by what it shows** (#5614): the words a sighted person reads stay in its
              name, which the rest of the sentence, heard and not drawn, completes (WCAG 2.5.3).
              One run of text laid out as if it were not there, so its spaces are heard between
              the pills and never drawn. */}
          <span className="skill-run-said">
            <span className="count">
              <Sparkles aria-hidden="true" />
              <span className="sr-only">Using the skill</span>{" "}
              <span className="skill-run-name">{run.name}</span>
              {copy !== undefined && (
                <>
                  {" "}
                  <span className="sr-only">{copy}</span>
                </>
              )}
            </span>
            <span className="sr-only">,</span> <span className="count">{run.tool_calls}</span>{" "}
            <span className="sr-only">{run.tool_calls === 1 ? "tool call" : "tool calls"}</span>{" "}
            {tokens !== undefined && (
              <>
                <span className="sr-only">and</span>{" "}
                <span className="count skill-run-tokens" data-testid="pane-skill-run-tokens">
                  {tokens}
                </span>{" "}
              </>
            )}
            <span className="sr-only">so far. Open its runs</span>
          </span>
        </Button>
      </span>
    </span>
  );
});
