// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (#2935): stand-in data shaped like the charter project's own plane. */

export type Scope = { persona?: string; workspace?: string; clone?: string };

export type Level = "Built in" | "Project" | "Workspace" | "Persona" | "Repo";

type Workspace = { name: string; clones: readonly { clone: string; repo: number }[] };

export const WORKSPACES: readonly Workspace[] = [
  { name: "smart-ide", clones: [{ clone: "purlis", repo: 12 }, { clone: "charter-app", repo: 3 }] },
  { name: "billing", clones: [] },
  { name: "docs-site", clones: [{ clone: "site", repo: 2 }] },
  { name: "infra", clones: [] },
];

/** Each persona, its role, and the workspaces it has been dispatched into, with how many of its
 *  chats ran there (what `in_the_studio(…, workspace, persona)` answers for). */
export const PERSONAS = [
  { name: "steward", role: "Control plane steward", into: [["smart-ide", 41], ["billing", 3]] },
  { name: "reviewer", role: "Code reviewer", into: [["smart-ide", 18]] },
  { name: "devops", role: "Infrastructure and CI", into: [["infra", 12], ["smart-ide", 2]] },
  { name: "writer", role: "Docs and changelog", into: [["docs-site", 9]] },
  { name: "qa", role: "Scenario specs", into: [] },
] as const satisfies readonly { name: string; role: string; into: readonly (readonly [string, number])[] }[];

/** Twelve personas: enough for a list to be filtered. */
export const MANY_PERSONAS = [
  ...PERSONAS,
  ...["designer", "triager", "release", "security", "data", "support", "mobile"].map((name) => ({
    name,
    role: "",
    into: [] as readonly (readonly [string, number])[],
  })),
];

export type Persona = (typeof MANY_PERSONAS)[number];

const PROJECT = ["grilling", "domain-modeling", "prototype", "tdd", "research", "code-review", "landing-a-pr", "report-intake"];
const BUILT_IN = ["purlis-remember", "purlis-handoff", "purlis-repo-work", "purlis-secrets", "purlis-smart-close"];
const BY_WORKSPACE: Record<string, string[]> = {
  "smart-ide": ["purlis-building-ui", "story-baselines", "mutants-read"],
  billing: ["stripe-webhooks"],
  "docs-site": ["docs-style"],
  infra: ["terraform-plan", "runner-image"],
};
const BY_PERSONA: Record<string, string[]> = {
  steward: ["backlog-sync", "wayfinder", "dispatch-brief"],
  reviewer: ["review-standards", "review-spec"],
  devops: ["ci-triage", "release-train"],
  writer: ["humanizer", "changelog-fragment"],
  qa: ["scenario-spec"],
};

export function skillsOf(scope: Scope): { name: string; level: Level }[] {
  const rows: { name: string; level: Level }[] = [
    ...PROJECT.map((name) => ({ name, level: "Project" as const })),
  ];
  if (scope.workspace) rows.push(...(BY_WORKSPACE[scope.workspace] ?? []).map((name) => ({ name, level: "Workspace" as const })));
  if (scope.persona) rows.push(...(BY_PERSONA[scope.persona] ?? ["persona-notes"]).map((name) => ({ name, level: "Persona" as const })));
  if (scope.clone) {
    const n = WORKSPACES.flatMap((one) => one.clones).find((one) => one.clone === scope.clone)?.repo ?? 0;
    rows.push(...Array.from({ length: Math.min(n, 4) }, (_, i) => ({ name: `${scope.clone}-skill-${i + 1}`, level: "Repo" as const })));
  }
  rows.push(...BUILT_IN.map((name) => ({ name, level: "Built in" as const })));
  return rows;
}

export function countOf(scope: Scope): number {
  const clone = WORKSPACES.flatMap((one) => one.clones).find((one) => one.clone === scope.clone);
  return skillsOf(scope).length - Math.min(clone?.repo ?? 0, 4) + (clone?.repo ?? 0);
}

/** The scope in words, as the trigger and the page's head say it. */
export function said(scope: Scope): string {
  const who = scope.persona ? `A chat as ${scope.persona}` : "A chat";
  const where = scope.workspace
    ? scope.clone
      ? `in ${scope.workspace} › ${scope.clone}`
      : `in ${scope.workspace}`
    : "at the project's root";
  return `${who} ${where}`;
}

export function key(scope: Scope): string {
  return [scope.persona ?? "", scope.workspace ?? "", scope.clone ?? ""].join("|");
}

export function fromKey(text: string): Scope {
  const [persona, workspace, clone] = text.split("|");
  return {
    ...(persona ? { persona } : {}),
    ...(workspace ? { workspace } : {}),
    ...(clone ? { clone } : {}),
  };
}

export function clonesOf(workspace: string | undefined) {
  return WORKSPACES.find((one) => one.name === workspace)?.clones ?? [];
}

export function dispatchedInto(persona: string | undefined, personas: readonly Persona[]) {
  return personas.find((one) => one.name === persona)?.into ?? [];
}
