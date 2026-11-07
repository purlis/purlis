// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A secret's one notice** (#5080, spec #5235, ADR 0112): every state of the reviewed prototype
 * of #5227, drawn on the ask's card as the Inbox draws it, from the `secret-ask` family. The
 * answers are faked at the notice's edge: a story reads which press it sent and what was given.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import {
  AHEAD,
  ENTRIES,
  FILL_ONLY,
  HOSTILE_COMMAND,
  INVISIBLE,
  ONCE_ONLY,
  VAULTS_OWN,
  runAsk,
  secretAsk,
} from "./secretAsk.testkit";
import { SecretAskNotice } from "./SecretAskNotice";
import { expectFocusInSight, inAWindow, untilFocused } from "./stories.testkit";
import { Card } from "./ui/card/Card";

/** The card the Inbox draws an ask on, around the notice. */
function TheCard(props: Parameters<typeof SecretAskNotice>[0]) {
  return (
    <Card variant="ask" className="inbox-ask">
      <SecretAskNotice {...props} />
    </Card>
  );
}

/** **The card in the Inbox's panel**: as wide as the panel, in a list that scrolls, so the action
 *  bar is held at the panel's foot as it is in the window, and the picture is the panel's top. */
function OnTheCard(props: Parameters<typeof SecretAskNotice>[0]) {
  return (
    <div style={{ inlineSize: 420, maxBlockSize: 720, overflow: "auto" }}>
      <TheCard {...props} />
    </div>
  );
}

const meta = {
  title: "Notices/One notice for a secret",
  component: OnTheCard,
  args: {
    ask: runAsk(),
    chat: "developer 7",
    entries: () => Promise.resolve(ENTRIES),
    busy: false,
    onAnswer: fn(),
    onSaid: fn(),
  },
} satisfies Meta<typeof OnTheCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **The panel scrolled to its end**, for a story whose subject is the notice's last line: the
 *  bar then stands at the card's own foot, under it. */
async function toTheEnd(canvasElement: HTMLElement) {
  // Once the faces are in, so the list is as tall as the picture will find it.
  await document.fonts.ready;
  await new Promise((frame) => requestAnimationFrame(frame));
  const list = canvasElement
    .querySelector<HTMLElement>('[data-testid="secret-ask"]')
    ?.closest("div[style]");
  if (list instanceof HTMLElement) list.scrollTop = list.scrollHeight;
  await new Promise((frame) => requestAnimationFrame(frame));
}

/** The press the story's notice sent last: its option's id and what it gave. */
function pressed(onAnswer: unknown) {
  const calls = (onAnswer as ReturnType<typeof fn>).mock.calls;
  const [option, given] = calls[calls.length - 1] as [{ id: string }, () => unknown];
  return { id: option.id, given: given() };
}

/** The full one notice: a vault the persona may not use, a missing key, an undeclared program.
 *  Filled and allowed once by keyboard alone. */
export const TheOneNotice: Story = {
  args: { ask: runAsk({ not_its_vault: true }) },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByTestId("sa-vault-line")).toHaveTextContent(
      "Allow once and Allow for this chat let this program use it for that alone",
    );
    // The undeclared marks are Tab stops, so their tooltips say why by keyboard too.
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByTestId("sa-undeclared-program"),
      "the program's undeclared mark",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByTestId("sa-undeclared"), "the key's undeclared mark");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("radio", { name: "New value" }),
      "the field's first way",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByLabelText("Value of STRIPE_API_KEY"), "the value's box");
    expectFocusInSight();
    await userEvent.keyboard("sk_test_v4lue");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("checkbox", { name: "Remember this need for the persona" }),
      "Remember",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("radio", { name: "Allow once" }),
      "the Allow, narrowest chosen",
    );
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Not now" }), "Not now");
    await userEvent.tab();
    const action = canvas.getByRole("button", { name: "Fill and allow once" });
    await untilFocused(() => action, "the one action");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onAnswer).toHaveBeenCalled());
    const sent = pressed(args.onAnswer);
    await expect(sent.id).toBe("allow-once");
    await expect(sent.given).toEqual({
      run: { values: [{ key: "STRIPE_API_KEY", value: "sk_test_v4lue" }], links: [], remember: [] },
    });
    await expect(canvas.getByLabelText("Value of STRIPE_API_KEY")).toHaveValue("");
  },
};

/** Several missing keys: a field per key, one Allow. */
export const SeveralKeys: Story = {
  args: {
    ask: runAsk({
      values: [
        { var: "KIBANA_USER", key: "KIBANA_USER", in_a_file: false },
        { var: "KIBANA_PASS", key: "KIBANA_PASS", in_a_file: true },
      ],
      fills: [
        { key: "KIBANA_USER", need: true },
        { key: "KIBANA_PASS", declared: "signing in to the staging dashboards", need: false },
      ],
      command:
        'purlis secret exec team --env KIBANA_USER=KIBANA_USER --file KIBANA_PASS=KIBANA_PASS -- curl -u "$KIBANA_USER" https://kibana.internal/api/status',
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getAllByTestId("sa-key-fill")).toHaveLength(2);
    await expect(canvas.getByTestId("sa-declared")).toHaveTextContent(
      "signing in to the staging dashboards",
    );
  },
};

/** Allow for this chat: what it lets happen, and where it is removed. */
export const ScopeForThisChat: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Allow once" }));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => canvas.getByRole("radio", { name: "Allow for this chat" }),
      "the next scope",
    );
    expectFocusInSight();
    await expect(canvas.getByTestId("sa-scope-said")).toHaveTextContent("until the chat ends");
    await expect(canvas.getByTestId("sa-scope-said")).toHaveTextContent("Settings › Sandbox");
    await expect(
      canvas.getByRole("button", { name: "Fill and allow for this chat" }),
    ).toBeVisible();
    await toTheEnd(canvasElement);
  },
};

/** Always allow, nothing else of the persona's waiting: the line first, and the Allow moves with it. */
export const ScopeAlways: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Always allow" }));
    await expect(canvas.getByTestId("sa-tool-line")).toHaveTextContent('curl = { vault = "team"');
    await expect(canvas.getByTestId("sa-others-wait")).toHaveTextContent("moves with this line");
    await expect(canvas.getByTestId("sa-env-advice")).toBeVisible();
    await toTheEnd(canvasElement);
  },
};

/** Always allow, with other changes of the persona's pending: this line alone is approved. */
export const ScopeAlwaysOthersPending: Story = {
  args: {
    ask: runAsk({
      always: {
        line: 'curl = { vault = "team", as = "STRIPE_API_KEY env", key = "STRIPE_API_KEY" }',
        by_env: true,
        others_wait: true,
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Always allow" }));
    await expect(canvas.getByTestId("sa-others-wait")).toHaveTextContent(
      "This approves only this line: their notice stays up",
    );
    await toTheEnd(canvasElement);
  },
};

/** Don't allow: the value is still stored, and the press says so. Declined by keyboard alone. */
export const ScopeDont: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("sk_test_v4lue");
    await userEvent.click(canvas.getByRole("radio", { name: "Allow once" }));
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    await untilFocused(() => canvas.getByRole("radio", { name: "Don't allow" }), "Don't allow");
    await expect(canvas.getByTestId("sa-scope-said")).toHaveTextContent(
      "STRIPE_API_KEY is still stored",
    );
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Fill, don't allow" }),
      "the one action",
    );
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onAnswer).toHaveBeenCalled());
    await expect(pressed(args.onAnswer).id).toBe("keep");
    await toTheEnd(canvasElement);
  },
};

/** Not now declines all of it: nothing typed is sent. */
export const NotNow: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("sk_test_v4lue");
    await userEvent.click(canvas.getByRole("button", { name: "Not now" }));
    const sent = pressed(args.onAnswer);
    await expect(sent.id).toBe("keep");
    await expect(sent.given).toEqual({ run: { values: [], links: [], remember: [] } });
    await expect(canvas.getByLabelText("Value of STRIPE_API_KEY")).toHaveValue("");
  },
};

/** A program purlis has not reviewed for any arguments: Allow once and Don't allow alone. */
export const AllowOnceOnly: Story = {
  args: {
    ask: runAsk(
      {
        program: "/usr/local/bin/stripe",
        caution: null,
        always: null,
        command:
          "purlis secret exec team --env STRIPE_API_KEY=STRIPE_API_KEY -- stripe charges list --limit 1",
      },
      ONCE_ONLY,
    ),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const allow = canvas.getByRole("radiogroup", { name: "Allow stripe" });
    await expect(within(allow).getAllByRole("radio")).toHaveLength(2);
    await expect(within(allow).getByRole("radio", { name: "Allow once" })).toBeChecked();
    await expect(within(allow).getByRole("radio", { name: "Don't allow" })).toBeVisible();
  },
};

/** The key is held: the program's Allow alone. */
export const AllowOnly: Story = {
  args: { ask: runAsk({ fills: [], remember: null }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.queryByTestId("sa-key-fill")).toBeNull();
    await expect(canvas.getByRole("button", { name: "Allow once" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Not now" })).toBeNull();
    await toTheEnd(canvasElement);
  },
};

/** Asked ahead of its run (#5244): no command yet, Allow for this chat alone. */
export const AheadOfItsRun: Story = {
  args: { ask: runAsk({ ahead: true, command: "", cwd: "", values: [], always: null }, AHEAD) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByTestId("sa-ahead")).toHaveTextContent("None yet");
    await expect(canvas.queryByTestId("sa-command")).toBeNull();
    await toTheEnd(canvasElement);
  },
};

/** No program Allow, on a vault the persona may not use: the vault's line has its own Allow. */
export const TheVaultsOwnAllow: Story = {
  args: { ask: runAsk({ offer: "vault", not_its_vault: true, always: null }, VAULTS_OWN) },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const allow = canvas.getByRole("button", { name: "Allow devops to use this vault" });
    await expect(canvas.getByTestId("sa-vault-line")).toContainElement(allow);
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("v4lue");
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    await untilFocused(() => allow, "the vault's Allow, on its line");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onAnswer).toHaveBeenCalled());
    await expect(pressed(args.onAnswer).id).toBe("allow-vault");
  },
};

/** Policy offers no Allow: Fill still stores, and the panel says why. */
export const PolicyLock: Story = {
  args: {
    ask: runAsk(
      {
        offer: "fill",
        always: null,
        no_allow:
          "your administrator's policy turns off allowing a program here (run-grants). Only a persona's declared tools receive a value",
      },
      FILL_ONLY,
    ),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByTestId("sa-locked")).toHaveTextContent(
      "You can still store the value.",
    );
    await expect(canvas.getByRole("button", { name: "Fill" })).toBeVisible();
  },
};

/** An ask naming no program: Fill only. */
export const NoProgram: Story = {
  args: { ask: secretAsk() },
};

/** A `secret_ask` naming a program that runs what it is told: Fill only, saying why. */
export const Interpreter: Story = {
  args: {
    ask: secretAsk({
      no_allow:
        "python3 gets no Allow: python3 runs what it is told, so purlis never hands it a value",
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByTestId("sa-locked")).toHaveTextContent("python3 gets no Allow");
    await expect(canvas.queryByRole("radio", { name: "Allow once" })).toBeNull();
  },
};

/** Remember ticked: the exact lines, the purpose to edit, and a `vault:` line where the persona
 *  names none. Filled by keyboard alone. */
export const RememberTicked: Story = {
  args: {
    ask: secretAsk({
      remember: {
        file: "personas/devops/persona.md",
        need: true,
        purpose: "The next step of #5012 calls Stripe's test mode.",
        vault: "devops",
      },
    }),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("v4lue");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("checkbox", { name: "Remember this need for the persona" }),
      "Remember",
    );
    await userEvent.keyboard(" ");
    await expect(canvas.getByTestId("sa-vault-need")).toHaveTextContent("vault: devops");
    await userEvent.tab();
    await untilFocused(() => canvas.getByLabelText("What STRIPE_API_KEY is for"), "the purpose");
    expectFocusInSight();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Fill" }), "Fill");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(args.onAnswer).toHaveBeenCalled());
    await expect(pressed(args.onAnswer).given).toEqual({
      remember: "The next step of #5012 calls Stripe's test mode.",
      named: undefined,
      typed: "v4lue",
    });
  },
};

/** Remember on a run's notice: a line per field the persona does not declare, purpose blank. */
export const RememberOnARun: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      canvas.getByRole("checkbox", { name: "Remember this need for the persona" }),
    );
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("v4lue");
    await userEvent.click(canvas.getByRole("button", { name: "Fill and allow once" }));
    await expect(args.onSaid).toHaveBeenCalledWith(
      "Say what STRIPE_API_KEY is for, or untick Remember.",
    );
    await expect(args.onAnswer).not.toHaveBeenCalled();
  },
};

/** A hostile command renders inert: each control drawn as its name, on one flow, apart from the
 *  app's panel. */
export const HostileCommand: Story = {
  args: {
    ask: runAsk({ command: HOSTILE_COMMAND }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const command = canvas.getByTestId("sa-command");
    const marks = within(command)
      .getAllByTestId("sa-control")
      .map((one) => one.textContent);
    await expect(marks).toEqual(["LF", "LF", "ESC", "ESC", "CR", "U+202E", "U+202C", "U+200B"]);
    await expect(command.textContent).not.toMatch(INVISIBLE);
    await expect(canvas.getByTestId("sa-facts")).not.toContainElement(command);
  },
};

/** Replace: the chat says the value doesn't work, in its own words. */
export const Replace: Story = {
  args: {
    ask: secretAsk({
      declared: true,
      declared_purpose: "Runs the checkout tests against Stripe's test mode.",
      replace: "api.stripe.com answered 401: Invalid API Key provided.",
      remember: null,
    }),
  },
};

/** Replace a Linked entry: a new value goes where it reads. */
export const ReplaceLinked: Story = {
  args: {
    ask: secretAsk({
      declared: true,
      declared_purpose: "Runs the checkout tests against Stripe's test mode.",
      replace: "api.stripe.com answered 401: Invalid API Key provided.",
      reads: { vault: "team", key: "STRIPE_TEST_KEY" },
      remember: null,
    }),
  },
};

/** A broken link on a run's field. */
export const BrokenLink: Story = {
  args: {
    ask: runAsk({
      fills: [
        { key: "STRIPE_API_KEY", need: false, gone: { vault: "old-team", key: "STRIPE_TEST_KEY" } },
      ],
      remember: null,
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByTestId("sa-gone")).toHaveTextContent("which is gone");
  },
};

/** Use an entry I have: the picker, grouped by vault with a filter, and a pick sent as a link. */
export const PickAnEntry: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Use an entry I have" }));
    const picker = await canvas.findByRole("button", { name: /Entry STRIPE_API_KEY reads/ });
    await userEvent.click(picker);
    const page = within(canvasElement.ownerDocument.body);
    await userEvent.click(await page.findByRole("option", { name: "STRIPE_TEST_KEY" }));
    await expect(canvas.getByRole("status")).toHaveTextContent(
      "becomes a link to team/STRIPE_TEST_KEY",
    );
    await userEvent.click(canvas.getByRole("button", { name: "Fill and allow once" }));
    await expect(pressed(args.onAnswer).given).toEqual({
      run: {
        values: [],
        links: [{ field: "STRIPE_API_KEY", vault: "team", key: "STRIPE_TEST_KEY" }],
        remember: [],
      },
    });
  },
};

/** The picker open: the person's entries grouped by vault, with the filter on top (#5070). */
export const ThePickerOpen: Story = {
  parameters: { screenshot: "page" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Use an entry I have" }));
    const picker = await canvas.findByRole("button", { name: /Entry STRIPE_API_KEY reads/ });
    await userEvent.click(picker);
    const page = within(canvasElement.ownerDocument.body);
    // Every vault's entries are listed, grouped by vault; the list scrolls, so some sit below.
    await expect(await page.findByRole("option", { name: "STRIPE_TEST_KEY" })).toBeInTheDocument();
    await expect(page.getByRole("option", { name: "SENTRY_DSN" })).toBeInTheDocument();
    await document.fonts.ready;
    await new Promise((frame) => requestAnimationFrame(frame));
  },
};

/** Use an entry I have with nothing picked: the press says so, and sends nothing. */
export const PickNothingYet: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("radio", { name: "Use an entry I have" }));
    await userEvent.click(canvas.getByRole("button", { name: "Fill and allow once" }));
    await expect(args.onSaid).toHaveBeenCalledWith(
      "Pick an entry for STRIPE_API_KEY, or choose New value.",
    );
    await expect(args.onAnswer).not.toHaveBeenCalled();
  },
};

/** Change, in place, on a Secret ask's field: a new name and vault, which the chat hears. */
export const ChangeInPlace: Story = {
  args: { ask: secretAsk() },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      canvas.getByRole("button", { name: "Change where STRIPE_API_KEY is stored" }),
    );
    const name = canvas.getByLabelText("Name STRIPE_API_KEY is stored under");
    await userEvent.clear(name);
    await userEvent.type(name, "STRIPE_TEST_KEY");
    await expect(
      canvas.getByText("developer 7 hears the new name, STRIPE_TEST_KEY."),
    ).toBeVisible();
    await userEvent.click(canvas.getByLabelText("Value of STRIPE_API_KEY"));
    await userEvent.keyboard("v4lue");
    await userEvent.click(canvas.getByRole("button", { name: "Fill" }));
    await expect(pressed(args.onAnswer).given).toMatchObject({
      named: { vault: "devops", key: "STRIPE_TEST_KEY" },
      typed: "v4lue",
    });
  },
};

/** A run's field offers no Change: its key is the one its command reads (the person's ruling of
 *  2026-11-07). */
export const NoChangeOnARun: Story = {
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).queryByRole("button", { name: /^Change/ })).toBeNull();
  },
};

/** Not declared by this persona: the badge, with why on its tooltip. */
export const Undeclared: Story = {
  args: {
    ask: secretAsk({
      key: "TWILIO_AUTH_TOKEN",
      purpose:
        "The user asked me to send a test SMS from the staging notifier; I need Twilio's auth token for that.",
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByTestId("sa-undeclared")).toHaveTextContent(
      "not declared by this persona",
    );
  },
};

/** At 1280×820, in a band that scrolls: the last line stays clear of the action bar, and a focus
 *  moved there is in sight. */
export const InAnOrdinaryWindow: Story = {
  args: { ask: runAsk({ not_its_vault: true }) },
  render: (props) => (
    <div style={{ inlineSize: 420, maxBlockSize: "60vh", overflow: "auto" }} data-testid="band">
      <TheCard {...props} />
    </div>
  ),
  play: async ({ canvasElement }) => {
    await inAWindow(1280, 820);
    const canvas = within(canvasElement);
    const band = canvas.getByTestId("band");
    await document.fonts.ready;
    await new Promise((frame) => requestAnimationFrame(frame));
    band.scrollTop = band.scrollHeight;
    await waitFor(() => {
      const foot = canvas
        .getByTestId("sa-hears")
        .closest("[data-part]:not([data-part='hears'])")
        ?.getBoundingClientRect();
      const last = canvas.getByTestId("sa-scope-said").getBoundingClientRect();
      if (foot === undefined || last.bottom > foot.top)
        throw new Error(`the last line (${last.bottom}) runs under the bar (${foot?.top})`);
    });
    await userEvent.click(canvas.getByRole("radio", { name: "Allow once" }));
    await untilFocused(() => canvas.getByRole("radio", { name: "Allow once" }), "the Allow");
    expectFocusInSight();
  },
};
