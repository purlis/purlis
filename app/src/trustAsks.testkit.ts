// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Shown } from "./bindings";

/** When each clone of {@link TRUST_ALL_ASK} was made, in seconds since 1970. */
export const MADE = {
  api: Math.floor(Date.parse("2026-10-28T09:30:00Z") / 1000),
  web: Math.floor(Date.parse("2026-11-02T16:05:00Z") / 1000),
};

/** **Chat 2's refused mint, for a clone purlis has no record of** (R-330), as the core lists it. */
export const TRUST_CLONE_ASK: Shown = {
  session: 2,
  ask: "trust-clone:2:workspaces/alpha/api|github.com/acme/api",
  says: "Trust this clone so chats can use GitHub: workspaces/alpha/api was not cloned by purlis, and its remote is github.com/acme/api. Trust it only if you made this clone yourself.",
  options: [{ id: "press", label: "Trust this clone", allows: true }],
  source: "trust-clone",
  chain: ["steward 2"],
  answer: { via: "trust-clone", folder: "workspaces/alpha/api", place: "github.com/acme/api" },
};

/** **The project's one ask for its clones purlis has no record of** (R-336, R-337): no chat's. */
export const TRUST_ALL_ASK: Shown = {
  session: 0,
  ask: "trust-clones",
  says: "Trust these clones: purlis did not clone workspaces/alpha/repos/api (github.com/acme/api), workspaces/beta/repos/web (github.com/acme/web). Trust them only if you made them yourself.",
  options: [
    { id: "trust-all", label: "Trust all", allows: true },
    { id: "choose", label: "Choose…", allows: true },
  ],
  source: "trust-clones",
  chain: ["purlis"],
  answer: {
    via: "trust-clones",
    clones: [
      {
        workspace: "alpha",
        folder: "workspaces/alpha/repos/api",
        place: "github.com/acme/api",
        places: ["github.com/acme/api"],
        made: MADE.api,
      },
      {
        workspace: "beta",
        folder: "workspaces/beta/repos/web",
        place: "github.com/acme/web",
        places: ["github.com/acme/web"],
        made: MADE.web,
      },
    ],
  },
};
