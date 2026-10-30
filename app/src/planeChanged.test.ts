// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { PlaneAnswer, PlaneChange } from "./bindings";
import {
  concerns,
  concernsAView,
  memoryIn,
  personaIn,
  sessionsAt,
  skillsIn,
  skillSetsOfChats,
  todosIn,
  CURATIONS,
  GIT,
  INSTRUCTIONS,
  panelsOf,
  ROOT_PANELS,
  SETTINGS,
  SIDEBAR,
  VIEWS,
} from "./planeChanged";

/**
 * Which answer a change concerns is the core's question (`purlis_core::planechange::answers`,
 * whose tests hold the mapping). The window only names the answer it holds, and these say how a
 * name is matched against what the core sent.
 */
describe("whether what the core says moved concerns a reader of the plane (FD-10)", () => {
  it("concerns every reader when the core could not say what changed", () => {
    expect(concerns(null, SIDEBAR)).toBe(true);
    expect(concerns(undefined, panelsOf("alpha"))).toBe(true);
  });

  it("concerns nobody when nothing changed", () => {
    expect(concerns([], SIDEBAR)).toBe(false);
    expect(concerns([], VIEWS)).toBe(false);
  });

  it("concerns a reader whose answer the core named, and no other", () => {
    const told: PlaneAnswer[] = [{ answer: "sidebar" }, { answer: "git" }];
    expect(concerns(told, SIDEBAR)).toBe(true);
    expect(concerns(told, GIT)).toBe(true);
    expect(concerns(told, INSTRUCTIONS)).toBe(false);
    expect(concerns(told, CURATIONS)).toBe(false);
    expect(concerns(told, SETTINGS)).toBe(false);
    expect(concerns(told, ROOT_PANELS)).toBe(false);
    expect(concerns(told, panelsOf("alpha"))).toBe(false);
  });

  it("keeps a workspace's panels to that workspace's, or every workspace's", () => {
    const beta: PlaneAnswer[] = [{ answer: "panels", workspace: "beta" }];
    expect(concerns(beta, panelsOf("beta"))).toBe(true);
    expect(concerns(beta, panelsOf("alpha"))).toBe(false);
    const every: PlaneAnswer[] = [{ answer: "panels", workspace: null }];
    expect(concerns(every, panelsOf("alpha"))).toBe(true);
    expect(concerns(every, panelsOf("beta"))).toBe(true);
  });

  it("reads the plane root's panels again when every workspace's moved, for its personas", () => {
    // #1686: the plane root draws the project's Personas panel, which a persona or the
    // project's file moves; a workspace's own panels do not concern it.
    expect(concerns([{ answer: "panels", workspace: null }], ROOT_PANELS)).toBe(true);
    expect(concerns([{ answer: "panels", workspace: "beta" }], ROOT_PANELS)).toBe(false);
  });

  it("tells the git readers what auto-save did, and nobody else", () => {
    const saved: PlaneAnswer[] = [{ answer: "git" }];
    expect(concerns(saved, GIT)).toBe(true);
    for (const other of [
      SIDEBAR,
      INSTRUCTIONS,
      CURATIONS,
      SETTINGS,
      ROOT_PANELS,
      VIEWS,
      panelsOf("alpha"),
    ])
      expect(concerns(saved, other)).toBe(false);
  });
});

/**
 * **A view tab follows what it shows, not every store** (#1160 L3): a memory's tab its own
 * store, a todo's tab its workspace's todos. Read only from what `plane-changed` already
 * carries: the core's `changes`, each with its kind and owner (`planechange::classify`).
 */
describe("whether a change concerns one view tab", () => {
  const VIEWS_MOVED: PlaneAnswer[] = [{ answer: "views" }];
  const change = (kind: PlaneChange["kind"], on: Partial<PlaneChange> = {}): PlaneChange => ({
    kind,
    workspace: null,
    persona: null,
    path: "",
    ...on,
  });
  const told = (changes: PlaneChange[] | null, answers: PlaneAnswer[] | null = VIEWS_MOVED) => ({
    changes,
    answers,
  });

  it("reads a memory's tab again for a memory of its own store, and for no other write", () => {
    const alpha = memoryIn({ kind: "workspace", name: "alpha" });
    expect(concernsAView(told([change("memory", { workspace: "alpha" })]), alpha)).toBe(true);
    expect(concernsAView(told([change("memory", { workspace: "beta" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("todos", { workspace: "alpha" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("sessions", { workspace: "alpha" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("memory", { persona: "alpha" })]), alpha)).toBe(false);
  });

  it("tells a persona's store from the shared one", () => {
    const steward = memoryIn({ kind: "persona", name: "steward" });
    const shared = memoryIn({ kind: "shared" });
    const own = told([change("memory", { persona: "steward" })]);
    const everyones = told([change("memory", { persona: "_shared" })]);
    expect(concernsAView(own, steward)).toBe(true);
    expect(concernsAView(own, shared)).toBe(false);
    expect(concernsAView(everyones, shared)).toBe(true);
    expect(concernsAView(everyones, steward)).toBe(false);
  });

  it("reads a todo's tab again for its workspace's todos alone", () => {
    const alpha = todosIn("alpha");
    expect(concernsAView(told([change("todos", { workspace: "alpha" })]), alpha)).toBe(true);
    expect(concernsAView(told([change("todos", { workspace: "beta" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("memory", { workspace: "alpha" })]), alpha)).toBe(false);
  });

  it("reads a memory's or todo's tab again when its workspace's own folder goes", () => {
    const memory = memoryIn({ kind: "workspace", name: "alpha" });
    const todos = todosIn("alpha");
    for (const about of [memory, todos]) {
      expect(concernsAView(told([change("workspace", { workspace: "alpha" })]), about)).toBe(true);
      expect(concernsAView(told([change("workspace", { workspace: null })]), about)).toBe(true);
      expect(concernsAView(told([change("workspace", { workspace: "beta" })]), about)).toBe(false);
    }
  });

  it("reads a persona's or the shared store's tab again when its persona's folder goes", () => {
    const steward = memoryIn({ kind: "persona", name: "steward" });
    const shared = memoryIn({ kind: "shared" });
    expect(concernsAView(told([change("persona", { persona: "steward" })]), steward)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: null })]), steward)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: "other" })]), steward)).toBe(false);
    expect(concernsAView(told([change("persona", { persona: "_shared" })]), shared)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: "steward" })]), shared)).toBe(false);
  });

  it("reads a session record's tab again for its own place's sessions alone (#1160 L1)", () => {
    const alpha = sessionsAt("workspaces/alpha/sessions/2026-10-02-1650-x.md");
    expect(concernsAView(told([change("sessions", { workspace: "alpha" })]), alpha)).toBe(true);
    expect(concernsAView(told([change("sessions", { workspace: "beta" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("sessions", { workspace: null })]), alpha)).toBe(false);
    expect(concernsAView(told([change("memory", { workspace: "alpha" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("todos", { workspace: "alpha" })]), alpha)).toBe(false);
    // Its workspace's folder going takes the record with it.
    expect(concernsAView(told([change("workspace", { workspace: "alpha" })]), alpha)).toBe(true);

    const root = sessionsAt("sessions/2026-10-02-1650-x.md");
    expect(concernsAView(told([change("sessions", { workspace: null })]), root)).toBe(true);
    expect(concernsAView(told([change("sessions", { workspace: "alpha" })]), root)).toBe(false);
    expect(concernsAView(told([change("persona", { persona: "steward" })]), root)).toBe(false);
  });

  it("reads a persona's record tab again for that persona's folder", () => {
    const steward = sessionsAt("personas/steward/sessions/2026-10-02-1650-x.md");
    expect(concernsAView(told([change("persona", { persona: "steward" })]), steward)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: null })]), steward)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: "other" })]), steward)).toBe(false);
    expect(concernsAView(told([change("sessions", { workspace: null })]), steward)).toBe(false);
  });

  it("follows every view store for a record path it cannot place", () => {
    expect(sessionsAt("elsewhere/x.md")).toBeUndefined();
    expect(sessionsAt("workspaces/alpha/notes/x.md")).toBeUndefined();
  });

  it("reads a persona's view again for that persona, its memory and the project (#1160 L1)", () => {
    const steward = personaIn("steward");
    expect(concernsAView(told([change("memory", { persona: "steward" })]), steward)).toBe(true);
    expect(concernsAView(told([change("memory", { persona: "other" })]), steward)).toBe(false);
    expect(concernsAView(told([change("memory", { persona: "_shared" })]), steward)).toBe(false);
    expect(concernsAView(told([change("memory", { workspace: "alpha" })]), steward)).toBe(false);
    expect(concernsAView(told([change("persona", { persona: "steward" })]), steward)).toBe(true);
    expect(concernsAView(told([change("persona", { persona: null })]), steward)).toBe(true);
    // Another persona's definition may be one this one inherits from: its facts read it.
    expect(concernsAView(told([change("persona", { persona: "other" })]), steward)).toBe(true);
    // The profiles the project offers are a fact of the view.
    expect(concernsAView(told([change("project")]), steward)).toBe(true);
    expect(concernsAView(told([change("todos", { workspace: "alpha" })]), steward)).toBe(false);
    expect(concernsAView(told([change("sessions", { workspace: "alpha" })]), steward)).toBe(false);
    expect(concernsAView(told([change("workspace", { workspace: "alpha" })]), steward)).toBe(false);
  });

  it("reads a persona's view again for a persona's or the project's skill, never a workspace's (SK-29)", () => {
    const steward = personaIn("steward");
    expect(concernsAView(told([change("skills", { persona: "steward" })]), steward)).toBe(true);
    expect(concernsAView(told([change("skills", { persona: "other" })]), steward)).toBe(true);
    expect(concernsAView(told([change("skills")]), steward)).toBe(true);
    expect(concernsAView(told([change("skills", { workspace: "alpha" })]), steward)).toBe(false);
  });

  it("reads the Skills view again for a skill it lists, and not another workspace's (SK-29)", () => {
    const alpha = skillsIn("alpha");
    const root = skillsIn(undefined);
    expect(concernsAView(told([change("skills")]), alpha)).toBe(true);
    expect(concernsAView(told([change("skills", { persona: "steward" })]), root)).toBe(true);
    expect(concernsAView(told([change("skills", { workspace: "alpha" })]), alpha)).toBe(true);
    expect(concernsAView(told([change("skills", { workspace: "beta" })]), alpha)).toBe(false);
    expect(concernsAView(told([change("skills", { workspace: "alpha" })]), root)).toBe(false);
  });

  it("reads the open chats' skill sets again for any level's skills or settings, and nothing else (#4094)", () => {
    for (const moved of [
      change("project"),
      change("harness"),
      change("persona", { persona: "steward" }),
      change("skills"),
      change("skills", { workspace: "beta" }),
      change("workspace", { workspace: "beta" }),
    ])
      expect(concernsAView(told([moved]), skillSetsOfChats)).toBe(true);
    for (const moved of [
      change("todos", { workspace: "alpha" }),
      change("memory", { workspace: "alpha" }),
      change("sessions"),
      change("chats"),
    ])
      expect(concernsAView(told([moved]), skillSetsOfChats)).toBe(false);
  });

  it("reads every tab again when the core could not say what changed", () => {
    const alpha = memoryIn({ kind: "workspace", name: "alpha" });
    expect(concernsAView(told(null), alpha)).toBe(true);
    expect(concernsAView(told(null, null), alpha)).toBe(true);
    expect(concernsAView(told([change("todos")], null), alpha)).toBe(true);
  });

  it("follows the views' answer as before for a tab with no finer interest", () => {
    expect(concernsAView(told([change("todos", { workspace: "alpha" })]), undefined)).toBe(true);
    expect(concernsAView(told([change("git")], [{ answer: "git" }]), undefined)).toBe(false);
  });
});
