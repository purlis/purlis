// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The empty state family** (#2913): in the middle of the content and at a side panel's width,
 * with its way out walked by the keyboard, and with none. Each held to axe and compared with its
 * baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { Inbox, ListTodo } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Button } from "../button/Button";
import { EmptyState, type EmptyStateVariant } from "./EmptyState";

/** An empty state whose way out says how many times it was taken. */
function WithAWayOut({ variant }: { variant?: EmptyStateVariant }) {
  const [opened, setOpened] = useState(0);
  return (
    <div style={{ inlineSize: variant === "panel" ? "16rem" : "32rem", blockSize: "16rem" }}>
      <EmptyState
        variant={variant}
        mark={Inbox}
        headline="No chats here yet"
        body={`Opened ${opened} ${opened === 1 ? "chat" : "chats"}`}
        action={
          <Button variant="primary" onPress={() => setOpened((was) => was + 1)}>
            Open a chat
          </Button>
        }
      />
    </div>
  );
}

const meta = {
  component: WithAWayOut,
} satisfies Meta<typeof WithAWayOut>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Tab reaches the way out with its ring in sight; Enter takes it. */
const takenByTheKeyboard: Story["play"] = async ({ canvas, userEvent }) => {
  const way = canvas.getByRole("button", { name: "Open a chat" });
  await userEvent.tab();
  await expect(way).toHaveFocus();
  expectFocusInSight();
  await userEvent.keyboard("{Enter}");
  await waitFor(() => expect(canvas.getByText("Opened 1 chat")).toBeInTheDocument());
  await userEvent.tab();
  await expect(way).not.toHaveFocus();
};

export const Page: Story = { args: { variant: "page" }, play: takenByTheKeyboard };
export const InAPanel: Story = { args: { variant: "panel" }, play: takenByTheKeyboard };

/** No way out is a legitimate empty state: it says what is true and offers nothing. */
export const NothingToDo: Story = {
  render: () => (
    <div style={{ inlineSize: "16rem" }}>
      <EmptyState variant="panel" mark={ListTodo} headline="No todos in this workspace" />
    </div>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByText("No todos in this workspace")).toBeInTheDocument();
    await expect(canvas.queryByRole("button")).toBeNull();
  },
};
