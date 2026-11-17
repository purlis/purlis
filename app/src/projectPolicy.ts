// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import type { PlaneId, PolicyOverride, PolicyState } from "./bindings";

/**
 * **The project's `[policy]` as the window last read it** (rulings Q308a, Q308b; ADR 0125;
 * instruction 45): one read per project, by the policy Notice (`ProjectPolicyNotice.tsx`), which
 * every project's view draws, and held here for the pane heads that say it. The window only says
 * the policy: the core resolves it, and a loosening still waits for the person's press.
 */
const held = new Map<PlaneId, PolicyState>();
const listeners = new Set<() => void>();

/** Holds what the core answered of `plane`'s policy, and tells whoever says it. */
export function holdPolicy(plane: PlaneId, state: PolicyState): void {
  held.set(plane, state);
  for (const listener of listeners) listener();
}

/** Forgets every project's policy: what a test starts from. */
export function forgetPolicies(): void {
  held.clear();
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** `plane`'s policy as last read, or `undefined` until it has been. */
export function useHeldPolicy(plane: PlaneId): PolicyState | undefined {
  return useSyncExternalStore(subscribe, () => held.get(plane));
}

/** The template's word, with "the default" where the file names none: "watch, the default". */
export function templateSaid(state: Pick<PolicyState, "template" | "template_default">): string {
  return state.template_default ? `${state.template}, the default` : state.template;
}

/** One override, as the window says it: "guards.merge enforce", and how it runs where that
 *  differs: "network off, enforced for now". */
export function overrideSaid(one: PolicyOverride): string {
  return `${one.family} ${modeSaid(one)}`;
}

/** An override's mode, and how it runs where that differs: "off, enforced for now". */
export function modeSaid(one: PolicyOverride): string {
  return one.runs === one.said ? one.said : `${one.said}, ${runsSaid(one.runs)} for now`;
}

const runsSaid = (mode: string): string =>
  mode === "enforce" ? "enforced" : mode === "log" ? "logged" : mode;

/**
 * **What the pane head says of the policy, in one sentence**: its template, and each family the
 * project sets apart from it. "Policy: watch, the default." or "Policy: team; set apart:
 * guards.release log."
 */
export function policySentence(state: PolicyState): string {
  const set =
    state.overrides.length === 0
      ? ""
      : `; set apart: ${state.overrides.map((one) => overrideSaid(one)).join(", ")}`;
  const waits =
    state.waiting.length > 0 ? " A loosening waits for your Accept in the project's view." : "";
  return `Policy: ${templateSaid(state)}${set}.${waits}`;
}
