// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Deliberate breaks to show the axe gate goes red (#2901); removed before merge.
import type { Meta, StoryObj } from "@storybook/react-vite";

function Gates({ kind }: { kind: "unnamed" | "small" }) {
  if (kind === "unnamed")
    return (
      <button type="button" tabIndex={0}>
        <svg aria-hidden="true" width="16" height="16" />
      </button>
    );
  return (
    <div style={{ display: "flex" }}>
      <button type="button" aria-label="One" style={{ width: 10, height: 10, padding: 0 }} />
      <button type="button" aria-label="Two" style={{ width: 10, height: 10, padding: 0 }} />
    </div>
  );
}

const meta = { component: Gates } satisfies Meta<typeof Gates>;
export default meta;

export const Unnamed: StoryObj<typeof meta> = { args: { kind: "unnamed" } };
export const SmallTargets: StoryObj<typeof meta> = { args: { kind: "small" } };
