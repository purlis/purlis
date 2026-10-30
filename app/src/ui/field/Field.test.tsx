// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The field family's wrapper** (#2905): what a person types, reads and hears, and the
 * attributes a theme's parts are keyed on (ADR 0103). The look itself is held by the stories, in
 * a real browser (`Field.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Search } from "lucide-react";
import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { drawnWith } from "../../cascade.testkit";
import { Button } from "../button/Button";
import { Form, TextArea, TextField, type PlainTextFieldProps } from "./Field";
import { submitKeySaid, submits } from "./submitKey";

afterEach(cleanup);

/** A field that holds its own value, so the keyboard can change it. */
function Held(props: Omit<PlainTextFieldProps, "value" | "onChange"> & { start?: string }) {
  const [value, setValue] = useState(props.start ?? "");
  return <TextField {...props} value={value} onChange={setValue} />;
}

/** ⌘⏎ on a Mac, Ctrl+Enter elsewhere: the platform the test runs on. */
const MAC = navigator.platform.startsWith("Mac");
const SEND = { key: "Enter", metaKey: MAC, ctrlKey: !MAC };

const NAME_RULE = (value: string) =>
  /^[\w.-]*$/.test(value) ? undefined : "A name is letters, digits, ., _ and -.";

/** The live line under the one field drawn: what is said of it, heard and described. */
const errorLine = () =>
  document.querySelector('[data-scope="field"][data-part="error"]:not([data-whole])') as Element;

describe("a text field", () => {
  it("is its label, its box, its help and its error, as one", () => {
    render(
      <TextField
        label="Name"
        help="It becomes a directory."
        error="That name is taken."
        value="docs"
        onChange={() => {}}
      />,
    );
    const box = screen.getByRole("textbox", { name: "Name" });
    expect(box.tagName).toBe("INPUT");
    expect(box).toHaveAccessibleDescription("It becomes a directory. That name is taken.");
    expect(box).toHaveAttribute("aria-invalid", "true");
    const root = box.closest('[data-scope="field"][data-part="root"]');
    expect(root).not.toBeNull();
    expect(root?.querySelector('[data-part="label"]')).toHaveTextContent("Name");
    expect(root?.querySelector('[data-part="help"]')).toHaveTextContent("It becomes a directory.");
    // Polite, never an alert as well (#3291).
    expect(root?.querySelector('[data-part="error"]')).toHaveAttribute("aria-live", "polite");
    expect(root?.querySelector('[data-part="error"]')).not.toHaveAttribute("role");
  });

  it("carries the text input's parts and its size, its glass hidden from a screen reader", () => {
    render(
      <TextField
        aria-label="Filter chats"
        type="search"
        size="lg"
        icon={<Search />}
        value=""
        onChange={() => {}}
      />,
    );
    const box = screen.getByRole("searchbox", { name: "Filter chats" });
    const control = box.parentElement;
    expect(control).toHaveAttribute("data-scope", "text-input");
    expect(control).toHaveAttribute("data-part", "root");
    expect(control).toHaveAttribute("data-size", "lg");
    const icon = control?.querySelector('[data-scope="text-input"][data-part="icon"]');
    expect(icon).toHaveAttribute("aria-hidden", "true");
    // The box is named by its label alone: no label part is drawn for it.
    expect(document.querySelector('[data-part="label"]')).toBeNull();
  });

  it("hands what is typed on as words, and a key on to its handler and beyond", async () => {
    const changed = vi.fn();
    const outside = vi.fn();
    const own = vi.fn();
    render(
      <div onKeyDown={(event) => outside(event.key)}>
        <TextField aria-label="Name" value="" onChange={changed} onKeyDown={(e) => own(e.key)} />
      </div>,
    );
    const box = screen.getByRole("textbox", { name: "Name" });
    await userEvent.type(box, "a");
    expect(changed).toHaveBeenLastCalledWith("a");
    fireEvent.keyDown(box, { key: "Escape" });
    expect(own).toHaveBeenLastCalledWith("Escape");
    // React Aria stops no key here: a dialog's Escape and a list's arrows still hear it.
    expect(outside).toHaveBeenLastCalledWith("Escape");
  });

  it("is never offered from the webview's history and never spell-checked, unless it asks", () => {
    render(<TextField aria-label="Path" value="" onChange={() => {}} />);
    const box = screen.getByRole("textbox", { name: "Path" });
    expect(box).toHaveAttribute("autocomplete", "off");
    expect(box).toHaveAttribute("spellcheck", "false");
  });

  it("is unavailable as React Aria says it, drawn so by its box", () => {
    render(<TextField aria-label="Name" isDisabled value="" onChange={() => {}} />);
    const box = screen.getByRole("textbox", { name: "Name" });
    expect(box).toBeDisabled();
    expect(box.parentElement).toHaveAttribute("data-disabled", "true");
  });

  it("is described by its hint for good while disabled, as a disabled button is (#2908)", () => {
    const why = "This project is read-only here.";
    render(
      <>
        <TextField aria-label="Skill" isDisabled tooltip={why} value="" onChange={() => {}} />
        <TextField aria-label="Secret" type="password" isDisabled tooltip={why} />
        <TextArea aria-label="Todo" isDisabled tooltip={why} value="" onChange={() => {}} />
        <TextField aria-label="Open" tooltip={why} value="" onChange={() => {}} />
      </>,
    );
    expect(screen.getByRole("textbox", { name: "Skill" })).toHaveAccessibleDescription(why);
    expect(screen.getByLabelText("Secret")).toHaveAccessibleDescription(why);
    expect(screen.getByRole("textbox", { name: "Todo" })).toHaveAccessibleDescription(why);
    // An enabled field's hint is its tooltip's alone, heard while it is shown.
    expect(screen.getByRole("textbox", { name: "Open" })).toHaveAccessibleDescription("");
  });
});

describe("a box that holds its own words (a secret's value)", () => {
  it("never puts what is typed in the page's markup, and is still the family's labelled box", async () => {
    const changed = vi.fn();
    render(
      <TextField
        label="Token"
        help="It goes to the keychain."
        type="password"
        onChange={changed}
      />,
    );
    const box = screen.getByLabelText("Token") as HTMLInputElement;
    expect(box).toHaveAccessibleDescription("It goes to the keychain.");
    await userEvent.type(box, "secret-123");
    expect(box.value).toBe("secret-123");
    expect(document.body.innerHTML).not.toContain("secret-123");
    expect(changed).toHaveBeenLastCalledWith(true);
    expect(box.parentElement).toHaveAttribute("data-scope", "text-input");
    expect(box.parentElement).toHaveAttribute("data-focus-within", "true");
  });
});

describe("inline validation (N26)", () => {
  it("says nothing while the person types, and what is wrong when they leave the box", async () => {
    render(<Held label="Name" validate={NAME_RULE} />);
    const box = screen.getByRole("textbox", { name: "Name" });
    await userEvent.type(box, "my name");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(box).not.toHaveAttribute("aria-invalid");
    await userEvent.tab();
    expect(errorLine()).toHaveTextContent("A name is letters, digits, ., _ and -.");
    expect(box).toHaveAttribute("aria-invalid", "true");
    expect(box).toHaveAccessibleDescription("A name is letters, digits, ., _ and -.");
  });

  it("says nothing for a box the person only tabbed through", async () => {
    render(
      <>
        <Held
          label="Colour"
          validate={(v) => (v === "" ? "A colour is # and six digits." : undefined)}
        />
        <Button>After</Button>
      </>,
    );
    await userEvent.tab();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    expect(screen.getByRole("textbox", { name: "Colour" })).not.toHaveAttribute("aria-invalid");
    expect(document.querySelector('[data-part="error"]')).toBeEmptyDOMElement();
  });

  it("keeps its error's line, an empty live region from the start, so a refusal moves nothing", async () => {
    render(<Held label="Name" validate={NAME_RULE} />);
    const line = document.querySelector('[data-scope="field"][data-part="error"]') as Element;
    // In the accessibility tree before it says anything (#3033): a live region that is already
    // there when its words land is announced by VoiceOver on WebKit. Polite (#3291): it waits
    // for the typing echo, and is never an alert as well.
    expect(line).toHaveAttribute("aria-live", "polite");
    expect(line).toHaveAttribute("aria-atomic", "true");
    expect(line).not.toHaveAttribute("aria-hidden");
    expect(line).not.toHaveAttribute("role");
    expect(line).toBeEmptyDOMElement();
    const box = screen.getByRole("textbox", { name: "Name" });
    expect(box).not.toHaveAccessibleDescription();
    await userEvent.type(box, "my name");
    await userEvent.tab();
    // The same element, now saying why: no part comes or goes, so the field keeps its height.
    expect(document.querySelector('[data-part="error"]')).toBe(line);
    expect(line).toHaveTextContent("A name is letters");
    // One signal (#3291): a live region whose words change, never an alert in the same render.
    expect(line).not.toHaveAttribute("role");
    expect(line).toHaveAttribute("aria-live", "polite");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(box).toHaveAccessibleDescription("A name is letters, digits, ., _ and -.");
    expect(drawnWith(line, "block-size")).toBe(
      "calc(var(--font-size-xs) * var(--line-height-body))",
    );
    expect(drawnWith(line.firstElementChild as Element, "white-space")).toBe("nowrap");
  });

  it("follows the typing once said, until the value is right", async () => {
    render(<Held label="Name" validate={NAME_RULE} start="my nam" />);
    const box = screen.getByRole("textbox", { name: "Name" });
    await userEvent.click(box);
    await userEvent.keyboard("e");
    await userEvent.tab();
    expect(box).toHaveAccessibleDescription("A name is letters, digits, ., _ and -.");
    await userEvent.click(box);
    await userEvent.clear(box);
    await userEvent.type(box, "my-name");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(box).not.toHaveAttribute("aria-invalid");
  });

  it("validates a textarea the same way", async () => {
    function Lines() {
      const [value, setValue] = useState("");
      return (
        <TextArea
          label="Hosts"
          value={value}
          onChange={setValue}
          validate={(v) => (v.includes(" ") ? "One host per line." : undefined)}
        />
      );
    }
    render(<Lines />);
    const box = screen.getByRole("textbox", { name: "Hosts" });
    expect(box.tagName).toBe("TEXTAREA");
    expect(box.parentElement).toHaveAttribute("data-scope", "textarea");
    await userEvent.type(box, "a b");
    await userEvent.tab();
    expect(errorLine()).toHaveTextContent("One host per line.");
  });
});

/** A form with a name and a note, and what it was sent with. */
function Sending({
  sent,
  ready = true,
  children,
}: {
  sent: (name: string) => void;
  ready?: boolean;
  children?: ReactNode;
}) {
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  return (
    <Form
      aria-label="New workspace"
      onSubmit={(event) => {
        event.preventDefault();
        sent(name);
      }}
    >
      <TextField label="Name" value={name} onChange={setName} validate={NAME_RULE} />
      <TextArea label="Note" value={note} onChange={setNote} />
      {children}
      <Button type="submit" variant="primary" isDisabled={!ready}>
        Create
      </Button>
    </Form>
  );
}

describe("a form (N26)", () => {
  it("sends its primary action on ⌘⏎ from a box and from a textarea, as Enter in a box does", async () => {
    const sent = vi.fn();
    render(<Sending sent={sent} />);
    const name = screen.getByRole("textbox", { name: "Name" });
    await userEvent.type(name, "docs");
    fireEvent.keyDown(name, SEND);
    expect(sent).toHaveBeenLastCalledWith("docs");
    const note = screen.getByRole("textbox", { name: "Note" });
    fireEvent.keyDown(note, SEND);
    expect(sent).toHaveBeenCalledTimes(2);
    // A plain Enter in the textarea is a new line, never a send.
    fireEvent.keyDown(note, { key: "Enter" });
    expect(sent).toHaveBeenCalledTimes(2);
  });

  it("sends nothing on ⌘⏎ while its primary action is unavailable", () => {
    const sent = vi.fn();
    render(<Sending sent={sent} ready={false} />);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), SEND);
    expect(sent).not.toHaveBeenCalled();
  });

  // #3636: an unavailable submit button that keeps the focus is still the form's primary action.
  for (const [way, off] of [
    ["made unavailable by hand", { "aria-disabled": true }],
    ["disabled with a reason", { isDisabled: true, tooltip: "Type an address first." }],
  ] as const)
    it(`sends nothing on ⌘⏎ while its submit button is ${way}`, () => {
      const sent = vi.fn();
      render(
        <Form aria-label="Go to an address" onSubmit={(e) => (e.preventDefault(), sent())}>
          <TextField label="Address" value="" onChange={() => {}} />
          <Button type="submit" {...off}>
            Go
          </Button>
        </Form>,
      );
      expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("type", "submit");
      fireEvent.keyDown(screen.getByRole("textbox", { name: "Address" }), SEND);
      expect(sent).not.toHaveBeenCalled();
    });

  it("never gives a grant or what cannot be undone on ⌘⏎: those are pressed", () => {
    for (const variant of ["grants", "danger"] as const) {
      const sent = vi.fn();
      render(
        <Form aria-label="Answer" onSubmit={(e) => (e.preventDefault(), sent())}>
          <TextField label="Why" value="" onChange={() => {}} />
          <Button type="submit" variant={variant}>
            {variant === "grants" ? "Allow" : "Delete"}
          </Button>
        </Form>,
      );
      fireEvent.keyDown(screen.getByRole("textbox", { name: "Why" }), SEND);
      expect(sent).not.toHaveBeenCalled();
      cleanup();
    }
  });

  it("says what is wrong at each field and takes the focus to the first, instead of sending", async () => {
    const sent = vi.fn();
    render(<Sending sent={sent} />);
    const name = screen.getByRole("textbox", { name: "Name" });
    await userEvent.type(name, "my name");
    const note = screen.getByRole("textbox", { name: "Note" });
    note.focus();
    // Moving from the box said it already; ⌘⏎ from the note sends nothing and goes back to it.
    await act(async () => {
      fireEvent.keyDown(note, SEND);
    });
    expect(sent).not.toHaveBeenCalled();
    expect(name).toHaveFocus();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Name: A name is letters, digits, ., _ and -.",
    );
    expect(name).toHaveAccessibleDescription("A name is letters, digits, ., _ and -.");
  });

  it("checks a field the person never left before it is sent", async () => {
    const sent = vi.fn();
    function Untouched() {
      return (
        <Form aria-label="Rename" onSubmit={(e) => (e.preventDefault(), sent())}>
          <TextField label="Name" value="a b" onChange={() => {}} validate={NAME_RULE} />
          <Button type="submit">Rename</Button>
        </Form>
      );
    }
    render(<Untouched />);
    await userEvent.click(screen.getByRole("button", { name: "Rename" }));
    expect(sent).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Name: A name is letters, digits, ., _ and -.",
    );
  });
});

/** A form with two fields that refuse, each a line of its own. */
function TwoRefusing({ sent }: { sent: () => void }) {
  const [name, setName] = useState("a b");
  const [port, setPort] = useState("http");
  return (
    <Form aria-label="New host" onSubmit={(e) => (e.preventDefault(), sent())}>
      <TextField label="Name" value={name} onChange={setName} validate={NAME_RULE} />
      <TextField
        label="Port"
        value={port}
        onChange={setPort}
        validate={(v) => (/^\d+$/.test(v) ? undefined : "A port is a number.")}
      />
      <Button type="submit">Add</Button>
    </Form>
  );
}

describe("one signal per refusal (#3291)", () => {
  const lines = () => [
    ...document.querySelectorAll('[data-scope="field"][data-part="error"]:not([data-whole])'),
  ];

  it("says a refused send once, as one alert summary, and each field keeps its own words", async () => {
    const sent = vi.fn();
    render(<TwoRefusing sent={sent} />);
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(sent).not.toHaveBeenCalled();
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    expect(alerts[0]).toHaveTextContent("2 fields to fix: Name, Port");
    // The fields' lines got their words in the same render: they are not heard over the summary.
    const [name, port] = lines();
    expect(name).toHaveTextContent("A name is letters");
    expect(port).toHaveTextContent("A port is a number.");
    for (const line of lines()) {
      expect(line).not.toHaveAttribute("role");
      expect(line).toHaveAttribute("aria-live", "off");
    }
    // Each field's error is reachable from its field.
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveAccessibleDescription(
      "A name is letters, digits, ., _ and -.",
    );
    expect(screen.getByRole("textbox", { name: "Port" })).toHaveAccessibleDescription(
      "A port is a number.",
    );
  });

  it("is heard field by field again, politely, once the person changes something", async () => {
    render(<TwoRefusing sent={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    const box = screen.getByRole("textbox", { name: "Port" });
    await userEvent.clear(box);
    await userEvent.type(box, "80");
    expect(screen.queryByRole("alert")).toBeNull();
    for (const line of lines()) expect(line).toHaveAttribute("aria-live", "polite");
    expect(lines()[1]).toBeEmptyDOMElement();
  });

  it("says the summary anew when the send is refused again", async () => {
    render(<TwoRefusing sent={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    const box = screen.getByRole("textbox", { name: "Port" });
    await userEvent.clear(box);
    await userEvent.type(box, "80");
    await userEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Name: A name is letters, digits, ., _ and -.",
    );
  });
});

describe("what a chord sends (N26)", () => {
  it("sends only a plain or suggested answer: any other look is pressed", () => {
    for (const [variant, sends] of [
      ["primary", true],
      ["secondary", true],
      ["soft-accent", false],
      ["warning-soft", false],
    ] as const) {
      const sent = vi.fn();
      render(
        <Form aria-label="Answer" onSubmit={(e) => (e.preventDefault(), sent())}>
          <TextField label="Why" value="" onChange={() => {}} />
          <Button type="submit" variant={variant}>
            Go
          </Button>
        </Form>,
      );
      fireEvent.keyDown(screen.getByRole("textbox", { name: "Why" }), SEND);
      expect(sent).toHaveBeenCalledTimes(sends ? 1 : 0);
      cleanup();
    }
  });

  it("never sends the form from what a portal draws outside it", () => {
    const sent = vi.fn();
    function Outer() {
      return (
        <Form aria-label="Outer" onSubmit={(e) => (e.preventDefault(), sent())}>
          <TextField label="Name" value="" onChange={() => {}} />
          {createPortal(
            <TextField label="Inside a dialog" value="" onChange={() => {}} />,
            document.body,
          )}
          <Button type="submit">Create</Button>
        </Form>
      );
    }
    render(<Outer />);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Inside a dialog" }), SEND);
    expect(sent).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), SEND);
    expect(sent).toHaveBeenCalledTimes(1);
  });
});

describe("the key that sends", () => {
  it("is ⌘⏎ on a Mac and Ctrl+Enter elsewhere, and never an Enter an input method composes", () => {
    const key = { key: "Enter", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false };
    expect(submits({ ...key, metaKey: true, isComposing: false }, true)).toBe(true);
    expect(submits({ ...key, ctrlKey: true, isComposing: false }, true)).toBe(false);
    expect(submits({ ...key, ctrlKey: true, isComposing: false }, false)).toBe(true);
    expect(submits({ ...key, metaKey: true, isComposing: false }, false)).toBe(false);
    expect(submits({ ...key, metaKey: true, isComposing: true }, true)).toBe(false);
    expect(submits({ ...key, metaKey: true, shiftKey: true, isComposing: false }, true)).toBe(
      false,
    );
    expect(submits({ ...key, isComposing: false }, true)).toBe(false);
    expect(submitKeySaid(true)).toBe("⌘↩");
    expect(submitKeySaid(false)).toBe("Ctrl+Enter");
  });
});

describe("a filter drawn again (R168)", () => {
  it("keeps the keyboard and the caret when its box is replaced", async () => {
    function Remounted() {
      const [value, setValue] = useState("");
      // The key changes with every letter: React draws a new box each time.
      return (
        <TextField
          key={value.length}
          aria-label="Filter personas"
          keepsFocus="story-filter"
          value={value}
          onChange={setValue}
        />
      );
    }
    render(<Remounted />);
    await userEvent.click(screen.getByRole("textbox", { name: "Filter personas" }));
    await userEvent.keyboard("abc");
    const box = screen.getByRole("textbox", { name: "Filter personas" }) as HTMLInputElement;
    expect(box).toHaveValue("abc");
    expect(box).toHaveFocus();
    expect([box.selectionStart, box.selectionEnd]).toEqual([3, 3]);
    // The caret moved back two: a redraw keeps it there.
    box.setSelectionRange(1, 1);
    await userEvent.keyboard("x");
    const again = screen.getByRole("textbox", { name: "Filter personas" }) as HTMLInputElement;
    expect(again).toHaveValue("axbc");
    expect(again).toHaveFocus();
    expect([again.selectionStart, again.selectionEnd]).toEqual([2, 2]);
  });

  it("never takes the keyboard back once the person has gone elsewhere", async () => {
    function Later({ shown }: { shown: boolean }) {
      return (
        <>
          <Button>Elsewhere</Button>
          {shown && (
            <TextField aria-label="Filter" keepsFocus="later" value="" onChange={() => {}} />
          )}
        </>
      );
    }
    const { rerender } = render(<Later shown />);
    screen.getByRole("textbox", { name: "Filter" }).focus();
    rerender(<Later shown={false} />);
    await new Promise((done) => setTimeout(done, 0));
    screen.getByRole("button", { name: "Elsewhere" }).focus();
    rerender(<Later shown />);
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
  });
});

describe("the family's default, through the window's cascade (field.css)", () => {
  it("draws a one-line box a control tall, a textarea three, and the ring outside either", () => {
    render(
      <>
        <TextField label="Name" value="" onChange={() => {}} />
        <TextArea label="Notes" value="" onChange={() => {}} />
      </>,
    );
    const box = screen.getByRole("textbox", { name: "Name" }).parentElement as Element;
    expect(drawnWith(box, "block-size")).toBe("var(--size-control)");
    const lines = screen.getByRole("textbox", { name: "Notes" }).parentElement as Element;
    expect(drawnWith(lines, "min-block-size")).toBe("calc(var(--size-control) * 3)");
    for (const one of [box, lines]) {
      one.setAttribute("data-focus-within", "true");
      expect(drawnWith(one, "outline")).toBe("var(--focus-ring-width) solid var(--focus-ring)");
    }
  });

  it("draws a refused box's edge in the danger colour, and only while it is refused", () => {
    render(
      <>
        <TextField aria-label="Colour" error="Not a colour." value="x" onChange={() => {}} />
        <TextField aria-label="Name" value="x" onChange={() => {}} />
      </>,
    );
    const refused = screen.getByRole("textbox", { name: "Colour" }).parentElement as Element;
    expect(drawnWith(refused, "border-color")).toBe("var(--danger-base)");
    const plain = screen.getByRole("textbox", { name: "Name" }).parentElement as Element;
    expect(drawnWith(plain, "border-color")).toBeUndefined();
  });

  it("draws an unavailable box's words in the faint tier, on its own ground", () => {
    render(<TextField aria-label="Host" isDisabled value="x" onChange={() => {}} />);
    const box = screen.getByRole("textbox", { name: "Host" }).parentElement as Element;
    expect(drawnWith(box, "color")).toBe("var(--text-faint)");
    expect(drawnWith(box, "background-color")).toBe("var(--surface-sunken)");
  });
});

describe("a refusal too long for its line (#3033)", () => {
  const LONG =
    "The core would not take this value: a workspace's sandbox profile names a mount outside " +
    "the workspace, and a profile may only mount what its workspace holds.";

  /** Every line in the window as `wide` for its words, `400` wide: a settings row is narrower
   *  than the core's words. */
  function sized(client: () => number) {
    const scroll = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(400);
    const width = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(client);
    return () => {
      scroll.mockRestore();
      width.mockRestore();
    };
  }
  const narrow = () => sized(() => 200);

  /** The room under a field: its live line and, beside it, the whole copy of a refusal cut short. */
  const roomOf = (line: Element) => line.parentElement as HTMLElement;
  const wholeOf = (line: Element) => roomOf(line).querySelector("[data-whole]");

  it("is described to the box in full, once, and the whole copy is for the eye alone", () => {
    const restore = narrow();
    try {
      render(<TextField aria-label="Profile" error={LONG} value="x" onChange={() => {}} />);
      const box = screen.getByRole("textbox", { name: "Profile" });
      expect(box).toHaveAttribute("aria-invalid", "true");
      expect(box).toHaveAccessibleDescription(LONG);
      const line = errorLine();
      expect(roomOf(line)).toHaveAttribute("data-clipped");
      const whole = wholeOf(line) as Element;
      expect(whole).toHaveTextContent(LONG);
      expect(whole).toHaveAttribute("aria-hidden", "true");
      // Beside the live line, never in it.
      expect(line.contains(whole)).toBe(false);
      expect(line.querySelector("[aria-hidden]")).toBeNull();
    } finally {
      restore();
    }
  });

  it("changes nothing in the live line when a resize cuts its words short or lets them fit", () => {
    let width = 600;
    const restore = sized(() => width);
    const watched: ResizeObserverCallback[] = [];
    const Was = globalThis.ResizeObserver;
    globalThis.ResizeObserver = class {
      constructor(callback: ResizeObserverCallback) {
        watched.push(callback);
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
    try {
      render(<TextField aria-label="Profile" error={LONG} value="x" onChange={() => {}} />);
      const line = errorLine();
      expect(wholeOf(line)).toBeNull();
      const changes: MutationRecord[] = [];
      const watch = new MutationObserver((records) => changes.push(...records));
      watch.observe(line, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
      const resized = () => act(() => watched.forEach((one) => one([], {} as ResizeObserver)));
      width = 200;
      resized();
      expect(wholeOf(line)).not.toBeNull();
      width = 600;
      resized();
      expect(wholeOf(line)).toBeNull();
      changes.push(...watch.takeRecords());
      watch.disconnect();
      expect(changes).toEqual([]);
    } finally {
      globalThis.ResizeObserver = Was;
      restore();
    }
  });

  it("draws its whole words over what follows while the field holds the keyboard, moving nothing", () => {
    const restore = narrow();
    try {
      render(<TextField aria-label="Profile" error={LONG} value="x" onChange={() => {}} />);
      const line = errorLine();
      const room = roomOf(line);
      const whole = wholeOf(line) as Element;
      expect(drawnWith(whole, "display")).toBe("none");
      // Under the pointer, on its room: the cascade's hover mark on the room itself. It takes
      // the pointer, so it stays while the pointer is on it.
      room.setAttribute("data-cascade-hover", "");
      expect(drawnWith(whole, "display", { hover: true })).toBe("block");
      expect(drawnWith(whole, "pointer-events", { hover: true })).toBe("auto");
      room.removeAttribute("data-cascade-hover");
      expect(drawnWith(whole, "display", { hover: true })).toBe("none");
      act(() => screen.getByRole("textbox", { name: "Profile" }).focus());
      expect(drawnWith(whole, "display")).toBe("block");
      // For the keyboard alone, a press goes through it to what it covers.
      expect(drawnWith(whole, "pointer-events")).toBe("none");
      expect(drawnWith(whole, "position")).toBe("absolute");
      expect(drawnWith(whole, "white-space")).toBe("normal");
      expect(drawnWith(whole, "background-color")).toBe("var(--surface-overlay)");
      // The edge a hint has, which forced colours keep.
      expect(drawnWith(whole, "border")).toBe("1px solid var(--border-subtle)");
      // The line keeps its one line: what is under the field never moves.
      expect(drawnWith(line, "block-size")).toBe(
        "calc(var(--font-size-xs) * var(--line-height-body))",
      );
    } finally {
      restore();
    }
  });

  it("is brought into sight when the field takes the keyboard", () => {
    const restore = narrow();
    const into = vi.fn();
    const had = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = into;
    try {
      render(<TextField aria-label="Profile" error={LONG} value="x" onChange={() => {}} />);
      expect(into).not.toHaveBeenCalled();
      act(() => screen.getByRole("textbox", { name: "Profile" }).focus());
      expect(into).toHaveBeenCalledWith({ block: "nearest" });
      expect(into.mock.contexts[0]).toBe(wholeOf(errorLine()));
    } finally {
      HTMLElement.prototype.scrollIntoView = had;
      restore();
    }
  });

  it("is drawn on its line alone when it fits, with no copy", () => {
    render(<TextField aria-label="Colour" error="Not a colour." value="x" onChange={() => {}} />);
    const line = errorLine();
    expect(roomOf(line)).not.toHaveAttribute("data-clipped");
    expect(wholeOf(line)).toBeNull();
  });

  it("reads whole in a box that holds its own words, as in one React Aria draws", () => {
    const restore = narrow();
    try {
      render(<TextField aria-label="Token" type="password" error={LONG} />);
      const box = screen.getByLabelText("Token");
      expect(box).toHaveAttribute("aria-invalid", "true");
      expect(box).toHaveAccessibleDescription(LONG);
      expect(roomOf(errorLine())).toHaveAttribute("data-clipped");
    } finally {
      restore();
    }
  });
});
