// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's own back and forward, and its path** (#3667, the Skill studio frame's top bar):
 * Library › deploy › Grades › New grade walked down by the pages' own buttons, then back and
 * forward by the keys (⌘[ and ⌘], or Alt+← and Alt+→ off a Mac) and by the path's names, each
 * button unavailable with its reason at its end. Held to axe in Chromium and WebKit.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { onAMac } from "../../tabKeys";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";
import { Panel } from "./Panel";
import { NO_BACK, ViewBar, type Crumb } from "./ViewBar";
import { ViewFrame, ViewListHead } from "./ViewFrame";
import { forgetViewHistory, useViewHistory } from "./viewHistory";

/** The pages under each, as the studio goes down a skill to a new grade. */
const NEXT: Record<string, string> = { Library: "deploy", deploy: "Grades", Grades: "New grade" };

/** A place: the path of page names from the Library. */
type Place = readonly string[];

function Studio({ tab }: { tab: string }) {
  const history = useViewHistory<Place>(tab, ["Library"]);
  const here = history.at.at(-1) ?? "Library";
  const next = NEXT[here];
  const path: Crumb[] = history.at.map((label) => ({ id: label, label }));
  return (
    <div
      style={{
        display: "grid",
        gridTemplateRows: "minmax(0, 1fr)",
        inlineSize: 760,
        blockSize: 320,
      }}
    >
      <Panel variant="content">
        <ViewFrame
          label="Skill studio pages"
          pages={[
            { id: "library", label: "Library" },
            { id: "sources", label: "Sources" },
          ]}
          page="library"
          onPage={() => undefined}
          history={history}
          bar={
            <ViewBar
              history={history}
              path={path}
              onGo={(_, at) => history.go(history.at.slice(0, at + 1))}
            >
              <Button size="sm">New skill…</Button>
            </ViewBar>
          }
          list={
            <ViewListHead title={here} lead="Where the person is, three levels down.">
              {next !== undefined && (
                <div>
                  <Button size="sm" onPress={() => history.go([...history.at, next])}>
                    {`Open ${next}`}
                  </Button>
                </div>
              )}
            </ViewListHead>
          }
        />
      </Panel>
    </div>
  );
}

const meta = {
  component: Studio,
  // Each story its own tab, whose history starts afresh.
  beforeEach: () => () => forgetViewHistory("studio"),
} satisfies Meta<typeof Studio>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The keys on this keyboard, as a person presses them. */
const BACK = () => (onAMac() ? "{Meta>}[[{/Meta}" : "{Alt>}{ArrowLeft}{/Alt}");
const FORWARD = () => (onAMac() ? "{Meta>}]{/Meta}" : "{Alt>}{ArrowRight}{/Alt}");

/** **Down three levels, then back and forward by the keys and by the path.** */
export const WalkThePath: Story = {
  args: { tab: "studio" },
  play: async ({ canvas, userEvent }) => {
    const path = () => canvas.getByRole("list", { name: "Path" });
    const here = () => path().querySelector('[aria-current="page"]')?.textContent;
    const back = canvas.getByRole("button", { name: "Back" });
    const forward = canvas.getByRole("button", { name: "Forward" });
    // At the start, neither goes anywhere, and each says why (it keeps the keyboard to say it).
    await expect(back).toHaveAttribute("aria-disabled", "true");
    await expect(forward).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await untilFocused(() => back, "back, unavailable");
    await waitFor(() => expect(document.body).toHaveTextContent(NO_BACK));
    // Down: deploy, Grades, New grade.
    for (const next of ["deploy", "Grades", "New grade"]) {
      await userEvent.click(canvas.getByRole("button", { name: `Open ${next}` }));
      await waitFor(() => expect(here()).toBe(next));
    }
    await expect(path().querySelectorAll("li")).toHaveLength(4);
    // Back twice by the keys, from inside the view.
    canvas.getByRole("button", { name: "New skill…" }).focus();
    await userEvent.keyboard(BACK());
    await waitFor(() => expect(here()).toBe("Grades"));
    await userEvent.keyboard(BACK());
    await waitFor(() => expect(here()).toBe("deploy"));
    await expect(forward).not.toHaveAttribute("aria-disabled");
    // Forward again by the key.
    await userEvent.keyboard(FORWARD());
    await waitFor(() => expect(here()).toBe("Grades"));
    // By the path: Library, a step of its own, which back undoes.
    await userEvent.click(within(path()).getByRole("button", { name: "Library" }));
    await waitFor(() => expect(here()).toBe("Library"));
    await expect(forward).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard(BACK());
    await waitFor(() => expect(here()).toBe("Grades"));
    // The bar's own buttons, by the keyboard.
    back.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(here()).toBe("deploy"));
    expectFocusInSight();
  },
};
