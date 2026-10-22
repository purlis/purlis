// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { SkillCompat, SkillFit } from "./bindings";

/** How well a skill fits, as its badge's word says it. */
export function fitSaid(fit: SkillFit): string {
  switch (fit) {
    case "full":
      return "full";
    case "partial":
      return "partial";
    case "widens":
      return "widens the chat";
    default:
      return "not known";
  }
}

/**
 * **A skill's compatibility badges** (SK-8, #2771; spec decision 18): one per harness that would
 * load it, saying how well it fits (full, partial, or widens the chat) and, opened, the core's
 * lines that say why: what applies, what purlis leaves out because a skill may only narrow a
 * chat, what is carried over by purlis's equivalent, and what is not applied. Nothing is
 * silently half-applied.
 */
export function SkillCompatBadges({ name, compat }: { name: string; compat: SkillCompat[] }) {
  if (compat.length === 0) return null;
  return (
    <ul className="skill-compat" aria-label={`Where ${name} works`}>
      {compat.map((one) => (
        <li key={one.harness}>
          <details
            className="skill-compat-badge"
            data-fit={one.fit}
            data-testid={`compat-${name}-${one.harness}`}
          >
            <summary>{`${one.title}: ${fitSaid(one.fit)}`}</summary>
            <ul>
              {one.says.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          </details>
        </li>
      ))}
    </ul>
  );
}
