// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **An org's skill controls in the Skill studio** (#6031, `prototype/skill-policy` A): each
 * source, skill and update says, on its own row and before any review, what the project's
 * `[skill_policy]` refuses of it, in the core's sentence, and a too-young update the day it can be
 * taken. One Notice above Sources says the controls apply, and See the controls opens Settings ›
 * Skill sources, which shows them read only. Every verdict is the core's.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type PolicyRefused,
  type SkillPolicy,
  type SourceOrigin,
  type SourcePolicy,
} from "./bindings";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { askSettingsAction } from "./settings/links";

/** The Settings group that shows the controls (#6031). */
export const SKILL_SOURCES_GROUP = "project.skill-sources";

/** What See the controls runs: Settings at Skill sources. */
export const SEE_THE_CONTROLS = `settings.group:${SKILL_SOURCES_GROUP}`;

const DAY = 24 * 60 * 60;

/** The day `until` (seconds since the epoch) falls on, as a row says it: Thursday 19 November. */
export function daySaid(until: number): string {
  return new Date(until * 1000).toLocaleDateString("en-GB", {
    weekday: "long",
    day: "numeric",
    month: "long",
  });
}

/** **A refusal's mark, in a word or two** (`prototype/skill-policy` A): what the row's badge says
 *  beside the core's sentence; a too-young one counts the days left from `now` (seconds). */
export function refusedMark(refused: PolicyRefused, now: number): string {
  switch (refused.kind) {
    case "blocked":
      return "Blocked";
    case "not-allowed":
      return "Not on the allow list";
    case "unread":
      return "Controls not read";
    case "too-young":
      return refused.until === null
        ? "Too young"
        : `Too young, ${counted(Math.max(1, Math.ceil((refused.until - now) / DAY)), "day", "days")} to go`;
  }
}

/** The core's sentence as a line of its own: one about the table itself starts "this project's",
 *  which a line starts in capitals. */
export function lineOf(why: string): string {
  return why.startsWith("this ") ? `T${why.slice(1)}` : why;
}

/** What a refused row says under its name: the core's sentence, and the day a too-young one can
 *  be taken. */
export function refusedSaid(refused: PolicyRefused): string {
  return refused.until === null
    ? `${lineOf(refused.why)}.`
    : `${lineOf(refused.why)}: it can be taken on ${daySaid(refused.until)}.`;
}

/** **A source's mark** (#6031): Blocked, Only <skills> allowed, Not on the allow list; none
 *  where the controls allow every skill of it. */
export function sourceMark(policy: SourcePolicy): string | undefined {
  if (policy.refused === null) return undefined;
  if (policy.kind === "blocked") return "Blocked";
  if (policy.kind === "unread") return "Controls not read";
  if (policy.only.length > 0) return `Only ${policy.only.join(", ")} allowed`;
  return "Not on the allow list";
}

/** Two origins are one source: the same repository and branch, or the same extension. */
function sameOrigin(a: SourceOrigin, b: SourceOrigin): boolean {
  if (a.kind === "git" && b.kind === "git")
    return a.remote.url === b.remote.url && a.remote.branch === b.remote.branch;
  return a.kind === "extension" && b.kind === "extension" && a.id === b.id;
}

/** What the controls do to `origin`, where they were asked about it. */
export function policyOf(
  policy: SkillPolicy | undefined,
  origin: SourceOrigin,
): SourcePolicy | undefined {
  return policy?.sources.find((one) => sameOrigin(one.origin, origin));
}

/** **The project's controls**, and what they do to each of `origins`, read again as `changed`
 *  moves; `undefined` until they answer, or where they could not be asked. */
export function useSkillPolicy(
  plane: string,
  origins: readonly SourceOrigin[],
  changed: number,
): SkillPolicy | undefined {
  const [policy, setPolicy] = useState<SkillPolicy>();
  const asked = JSON.stringify(origins);
  useEffect(() => {
    let live = true;
    void commands
      .skillPolicy(plane, JSON.parse(asked) as SourceOrigin[])
      .then((answer) => {
        if (live && answer.status === "ok") setPolicy(answer.data);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, asked, changed]);
  return policy;
}

/** **The one Notice above Sources** (#6031): this project's controls apply, in the core's line,
 *  or why they cannot be read; See the controls opens Settings › Skill sources. Nothing where the
 *  project has none. */
export function PolicyNotice({
  plane,
  policy,
}: {
  plane: string;
  policy: SkillPolicy | undefined;
}) {
  if (policy === undefined || policy.state.kind === "none") return null;
  const link = {
    label: "See the controls",
    onPress: () => askSettingsAction(plane, SEE_THE_CONTROLS),
  };
  return policy.state.kind === "refused" ? (
    <Notice cause="skill-policy-unread" tone="attention" link={link}>
      <span data-testid="skill-policy">{lineOf(policy.state.why)}.</span>
    </Notice>
  ) : (
    <Notice cause="skill-policy-applies" link={link}>
      <span data-testid="skill-policy">{policy.state.said}</span>
    </Notice>
  );
}
