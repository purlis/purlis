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
  type PolicyKind,
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

/** **What each of the controls' refusals is marked, in a word or two** (`prototype/skill-policy`
 *  A): the badge beside the core's sentence, on a source's row and on a skill's or an update's. */
const MARKS: Record<PolicyKind, string> = {
  blocked: "Blocked",
  "not-allowed": "Not on the allow list",
  "too-young": "Too young",
  undated: "Age not known",
  unread: "Controls not read",
};

/** A refusal's mark: a too-young one counts the days left from `now` (seconds). */
export function refusedMark(refused: PolicyRefused, now: number): string {
  if (refused.kind !== "too-young" || refused.until === null) return MARKS[refused.kind];
  const days = Math.max(1, Math.ceil((refused.until - now) / DAY));
  return `${MARKS["too-young"]}, ${counted(days, "day", "days")} to go`;
}

/** The core's sentence as a line of its own: one about the table itself starts "this project's",
 *  which a line starts in capitals. */
export function lineOf(why: string): string {
  return why.startsWith("this ") ? `T${why.slice(1)}` : why;
}

/** The core's sentence `why` as a line of its own, ended. */
export function sentenceOf(why: string): string {
  return `${lineOf(why)}.`;
}

/** What a refused row says under its name: the core's sentence, and the day a too-young one can
 *  be taken after the days the core counts ("…: it can be installed in 3 days, on Monday 16
 *  November."). */
export function refusedSaid(refused: PolicyRefused): string {
  return refused.until === null
    ? sentenceOf(refused.why)
    : `${lineOf(refused.why)}, on ${daySaid(refused.until)}.`;
}

/** **A source's mark** (#6031): Blocked, Only <skills> allowed, Not on the allow list; none
 *  where the controls allow every skill of it. */
export function sourceMark(policy: SourcePolicy): string | undefined {
  if (policy.refused === null) return undefined;
  if (policy.only.length > 0) return `Only ${policy.only.join(", ")} allowed`;
  return MARKS[policy.kind ?? "not-allowed"];
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

/** Each ask of the controls still on its way, by plane, change and origins: the rows drawn at once
 *  (Settings draws one per key) share one ask. Let go as it answers, so a later draw asks again. */
const asking = new Map<string, Promise<SkillPolicy | string>>();

/** The controls, or why they could not be read, in the core's words. */
function askPolicy(
  plane: string,
  origins: readonly SourceOrigin[],
  changed: number,
): Promise<SkillPolicy | string> {
  const key = `${plane}\u0000${changed}\u0000${JSON.stringify(origins)}`;
  const waiting = asking.get(key);
  if (waiting !== undefined) return waiting;
  const ask = commands
    .skillPolicy(plane, [...origins])
    .then(
      (answer) => (answer.status === "ok" ? answer.data : answer.error),
      (why: unknown) => `purlis could not read this project's skill controls: ${String(why)}`,
    )
    .finally(() => asking.delete(key));
  asking.set(key, ask);
  return ask;
}

/** **The project's controls**, and what they do to each of `origins`, read again as `changed`
 *  moves: `undefined` until they answer, and why where they could not be read. */
export function useSkillPolicyRead(
  plane: string,
  origins: readonly SourceOrigin[],
  changed: number,
): SkillPolicy | string | undefined {
  const [policy, setPolicy] = useState<SkillPolicy | string>();
  const asked = JSON.stringify(origins);
  useEffect(() => {
    let live = true;
    void askPolicy(plane, JSON.parse(asked) as SourceOrigin[], changed).then((answer) => {
      if (live) setPolicy(answer);
    });
    return () => {
      live = false;
    };
  }, [plane, asked, changed]);
  return policy;
}

/** [`useSkillPolicyRead`], where nothing is said of controls that could not be read: Sources draws
 *  its rows as it would without them, and each review still says why. */
export function useSkillPolicy(
  plane: string,
  origins: readonly SourceOrigin[],
  changed: number,
): SkillPolicy | undefined {
  const read = useSkillPolicyRead(plane, origins, changed);
  return typeof read === "object" ? read : undefined;
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
