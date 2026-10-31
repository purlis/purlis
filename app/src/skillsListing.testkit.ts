// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The Skill studio's answers, as the Skills view's tests and stories hand them (#3266). */
import type { NeedsYou, Studio, StudioSkill } from "./bindings";

/** A skill as the studio answers one: a project skill a chat gets, unless told otherwise. */
export function skill(name: string, over: Partial<StudioSkill> = {}): StudioSkill {
  return {
    name,
    level: "project",
    label: "project",
    local: false,
    grant: "root",
    granted: { to: "root" },
    turned_off: false,
    not_granted: null,
    on: true,
    left_out: null,
    accept: null,
    description: `What ${name} does.`,
    folder: `skills/${name}`,
    written: name,
    named: false,
    unchecked: null,
    owner: null,
    switch: null,
    compat: [],
    lint: [],
    hidden: [],
    pinned: false,
    pinned_by: [],
    ...over,
  };
}

/** **One item of Needs you** (#3689), a change to review unless `over` says otherwise. */
export function need(id: string, over: Partial<NeedsYou> = {}): NeedsYou {
  return {
    id,
    kind: "review",
    title: id,
    tag: "project",
    reason: "A chat made it while it ran. Chats get it once you accept it.",
    fix: { kind: "accept", folder: `skills/${id}` },
    fix_label: "Accept it",
    fix_does: "Commits it, so chats started after that get it.",
    ...over,
  };
}

/** A studio answer holding `skills`, and each program's own. */
export function studio(skills: StudioSkill[], over: Partial<Studio> = {}): Studio {
  return {
    on: true,
    workspace: null,
    persona: null,
    workspaces: ["ops"],
    levels: [{ level: { at: "project", local: false }, label: "project" }],
    skills,
    harnesses: [],
    clashes: [],
    first: { starts: false, unmoved: null },
    pins_refused: null,
    not_supported: [],
    levels_to_confirm: null,
    router: null,
    needs: [],
    ...over,
  };
}
