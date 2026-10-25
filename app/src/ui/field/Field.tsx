// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The field family** (#2905, spec #2897, ADR 0104): every box a person types in, on React
 * Aria's `TextField`, `Input`, `TextArea`, `Group`, `Label` and `Text`, wrapped here once, and the
 * form they are sent from. A screen
 * imports these and never `react-aria-components` (`eslint.config.js`), and draws no `<input>` or
 * `<textarea>` of its own (`purlis/field-family`, an error).
 *
 * - **A field is its label, its control, its help and its error, as one** (`TextField`,
 *   `TextArea`). The label names the control, the help and the error describe it, and React Aria
 *   ties them by id. A field with no visible label (a filter, a box in a table's cell, a row of
 *   the settings set, whose row draws the label) is named by `aria-label` or `aria-labelledby`.
 * - **Drawn by its parts.** The field's root carries `data-scope="field"`, and its `label`,
 *   `help` and `error` parts; its control is the `text-input` or `textarea` family's root, with
 *   the `icon` part at a text input's start. `field.css` draws the family default from those and
 *   React Aria's state attributes, and a theme's parts reach the same attributes (ADR 0103). A
 *   screen gives a field a place (`className` for margin, width, flex, order), never a look.
 * - **Sizes** (a text input): `sm` 24, `md` 28 (the default), `lg` 32, which is only for a list's
 *   main filter field.
 * - **It validates inline, on blur** (N26): `validate` says what is wrong with a value, and the
 *   field says it under the box when the person leaves it, then follows their typing until it is
 *   right. A refusal from elsewhere (`error`, the core's words) is said at once. Either marks the
 *   box invalid (`aria-invalid`) and describes it by the words.
 * - **⌘⏎ submits** (N26, Ctrl+Enter off macOS): a {@link Form} sends its primary action, its first
 *   submit button, from anywhere in it, a textarea included; a form whose fields are not yet right
 *   says why at each one and takes the focus to the first.
 * - **A filter keeps its focus and its caret** (R168): a field given `keepsFocus` that is drawn
 *   again (its list re-rendered, or the field itself replaced) keeps the keyboard and the caret
 *   where they were.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type DOMAttributes,
  type FocusEvent,
  type FormEvent,
  type FormHTMLAttributes,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  type Ref,
  type RefCallback,
} from "react";
import {
  Group,
  Input,
  Label,
  Text,
  TextArea as AriaTextArea,
  TextField as AriaTextField,
} from "react-aria-components";
import { submits } from "./submitKey";
import { Tooltip } from "../tooltip/Tooltip";

/** The text input's sizes: 24, 28 and 32 px at the default text size and density (`field.css`). */
export const FIELD_SIZES = ["sm", "md", "lg"] as const;
export type FieldSize = (typeof FIELD_SIZES)[number];

/** The native handlers a call site hands the element typed in, as React gives them. */
type Handlers<E> = Pick<
  DOMAttributes<E>,
  | "onKeyDown"
  | "onKeyUp"
  | "onFocus"
  | "onBlur"
  | "onPaste"
  | "onSelect"
  | "onClick"
  | "onPointerDown"
  | "onMouseDown"
  | "onCompositionStart"
  | "onCompositionEnd"
>;

/** What every field takes. */
interface Common<E extends HTMLElement> extends Handlers<E> {
  /** The words in the box, as typed. Left out, the box holds its own words and the caller reads
   *  them from it: a secret's value is never held in React state (a vault's token). */
  value?: string;
  onChange?: (value: string) => void;
  /** Its visible name, over the box. Without one, `aria-label` or `aria-labelledby` names it. */
  label?: ReactNode;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  /** What else describes the box: a question it answers, a line its row draws. */
  "aria-describedby"?: string;
  /** The box's own id, which a row's `<label for>` points at. */
  id?: string;
  /** One line under the box on what the value is for. */
  help?: ReactNode;
  /** What is wrong with a value, or nothing: said under the box when the person leaves it. */
  validate?: (value: string) => string | undefined;
  /** A refusal from elsewhere, in its own words, said at once: the core would not take it. */
  error?: string;
  /** Refused, where the words that say why are drawn elsewhere (a tab's rename, whose card is a
   *  popover) and named by `aria-describedby`. */
  isInvalid?: boolean;
  isDisabled?: boolean;
  isReadOnly?: boolean;
  isRequired?: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  name?: string;
  maxLength?: number;
  /** A path, a name or a token is never offered from the webview's history unless said. */
  autoComplete?: string;
  spellCheck?: boolean;
  inputMode?: "text" | "numeric" | "url" | "email" | "search";
  enterKeyHint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send";
  /** Its hint, the shared tooltip (#2908), on the element typed in. */
  tooltip?: string;
  /** Its place in a roving group's tab sequence, which the group hands it (Inbox's rows). */
  tabIndex?: number;
  /** `data-*` attributes reach the element typed in as they are: a test's id, a roving group's
   *  mark on its item. */
  [data: `data-${string}`]: string | number | boolean | undefined;
  /**
   * **A box that drives a list of its own** (the palette): its role and the ARIA that ties it to
   * the list, which the box hands on as they are. The list and its keys stay the caller's until
   * the combobox family moves (#2906).
   */
  role?: "combobox";
  "aria-expanded"?: boolean | "true" | "false";
  "aria-controls"?: string;
  "aria-activedescendant"?: string;
  "aria-autocomplete"?: "list" | "none" | "inline" | "both";
  /** A place for the field in its view: margin, width, flex, order. Never a look. */
  className?: string;
  style?: CSSProperties;
}

interface OneLine extends Common<HTMLInputElement> {
  /** Ignored for a list's main filter alone: `lg`. */
  size?: FieldSize;
  /** The mark at the box's start: a filter's or a search's glass. */
  icon?: ReactNode;
  /** The box's width in characters, for a box as wide as what it holds (a table's cell). */
  columns?: number;
  /** A `<datalist>`'s id: the suggestions the box offers. */
  list?: string;
  /** Keep the keyboard and the caret here when the field is drawn again (R168): a name unique to
   *  this field in the window, such as `chats-filter`. */
  keepsFocus?: string;
  inputRef?: Ref<HTMLInputElement>;
  /** The element typed in, as a group that wraps the field (`asChild`) asks for it. */
  ref?: Ref<HTMLInputElement>;
}

/**
 * A one-line field's props. **A secret's box (`type="password"`) is never given its words**: it
 * holds them itself, so they are never in React's state or the page's markup, and it says only
 * whether it is filled. Giving it a `value` is a type error.
 */
export type PlainTextFieldProps = OneLine & { type?: "text" | "search" | "url" | "email" };

export type TextFieldProps =
  | PlainTextFieldProps
  | (Omit<OneLine, "value" | "onChange" | "validate" | "keepsFocus"> & {
      type: "password";
      value?: never;
      validate?: never;
      keepsFocus?: never;
      /** Whether the box holds anything: never its words. */
      onChange?: (filled: boolean) => void;
    });

export interface TextAreaProps extends Omit<Common<HTMLTextAreaElement>, "value"> {
  /** A textarea's words are always the caller's. */
  value: string;
  rows?: number;
  /** As tall as its lines, and never dragged: a list of entries (DS-10). */
  grows?: boolean;
  inputRef?: Ref<HTMLTextAreaElement>;
  /** The element typed in, as a group that wraps the field (`asChild`) asks for it. */
  ref?: Ref<HTMLTextAreaElement>;
}

// ---------------------------------------------------------------------------------------------
// The form: ⌘⏎, and every field checked before it is sent
// ---------------------------------------------------------------------------------------------

/** A field a form checks before it is sent: whether its value is right, said where it is not. */
interface Check {
  check: () => boolean;
  element: () => HTMLElement | null;
}

const Checks = createContext<Set<Check> | null>(null);

/** A form's first submit button: its primary action, the one ⌘⏎ and Enter in a box send. */
function primaryOf(form: HTMLFormElement): HTMLButtonElement | HTMLInputElement | null {
  return form.querySelector<HTMLButtonElement | HTMLInputElement>(
    'button[type="submit"], input[type="submit"]',
  );
}

/** **The answers a chord may give**: the suggested answer and the plain one. Every other look
 *  (a grant, ADR 0097 §6; what cannot be undone, D-1719-3; any look added later) is only ever a
 *  press of its own button, so a new kind of answer is never sent by a chord until it is named
 *  here. */
const CHORD_SENDS = new Set(["primary", "secondary"]);

/**
 * Whether ⌘⏎ leaves a form's primary action alone: it says it cannot be pressed now (`disabled`,
 * `aria-disabled`), or it is not an answer a chord may give.
 */
function unavailable(button: HTMLElement): boolean {
  const variant = button.getAttribute("data-variant");
  return (
    (button as HTMLButtonElement).disabled ||
    button.getAttribute("aria-disabled") === "true" ||
    (variant !== null && !CHORD_SENDS.has(variant))
  );
}

export type FormProps = Omit<FormHTMLAttributes<HTMLFormElement>, "onSubmit" | "className"> & {
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /** A place for the form in its view. Never a look. */
  className?: string;
  ref?: Ref<HTMLFormElement>;
  children?: ReactNode;
};

/**
 * **A form** (N26): a native `<form>` whose fields are checked before it is sent, and which
 * ⌘⏎ (Ctrl+Enter off macOS) sends from anywhere in it, a textarea included, as Enter in a box
 * does. Its primary action is its first submit button; while that is unavailable, or when it
 * is a grant or what cannot be undone, ⌘⏎ sends nothing: those are pressed. A field that is not right says why and the focus goes to the first such, and
 * `onSubmit` is not called.
 */
export function Form({ onSubmit, onKeyDown, children, ...rest }: FormProps) {
  const [checks] = useState(() => new Set<Check>());
  const submit = (event: FormEvent<HTMLFormElement>) => {
    const wrong = [...checks]
      .filter((one) => !one.check())
      .map((one) => one.element())
      .filter((element): element is HTMLElement => element !== null)
      .sort((a, b) => (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
    if (wrong.length > 0) {
      event.preventDefault();
      wrong[0].focus();
      return;
    }
    onSubmit(event);
  };
  const keys = (event: KeyboardEvent<HTMLFormElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented || !submits(event.nativeEvent)) return;
    // A key from what a portal draws outside the form (a dialog, a list) is not the form's.
    if (!event.currentTarget.contains(event.target as Node)) return;
    event.preventDefault();
    const form = event.currentTarget;
    const primary = primaryOf(form);
    if (primary && unavailable(primary)) return;
    form.requestSubmit(primary ?? undefined);
  };
  return (
    <Checks.Provider value={checks}>
      {/* A native form: its fields check themselves (React Aria's `aria` validation), so the
          browser's own checks and bubbles are off. */}
      <form {...rest} noValidate onSubmit={submit} onKeyDown={keys}>
        {children}
      </form>
    </Checks.Provider>
  );
}

// ---------------------------------------------------------------------------------------------
// Inline validation
// ---------------------------------------------------------------------------------------------

/**
 * **What a field says is wrong** (N26): nothing until the person leaves a box whose words they
 * changed (a box only tabbed through says nothing); then what `validate` says, which follows
 * their typing from there until it is right. A refusal from elsewhere (`error`) is said at once
 * and wins. A form checks every field before it is sent, changed or not.
 */
function useInline(
  value: string | undefined,
  validate: ((value: string) => string | undefined) | undefined,
  error: string | undefined,
  element: () => HTMLInputElement | HTMLTextAreaElement | null,
) {
  const [shown, setShown] = useState<string | undefined>(undefined);
  // The words the box was drawn with: leaving it with the same words is no answer to check.
  const [first] = useState(value);
  // Once said, it follows the value: a fix takes it away, a new mistake says the new words. A box
  // that holds its own words is checked again only when it is left or its form is sent.
  const following = shown === undefined || value === undefined ? shown : validate?.(value);
  const check = useCallback(() => {
    // The box's own value: what the person sees in it when they leave it or send the form.
    const now = validate?.(element()?.value ?? value ?? "");
    setShown(now);
    return now === undefined;
  }, [validate, element, value]);
  const checks = useContext(Checks);
  useEffect(() => {
    if (!checks || !validate) return;
    const one: Check = { check, element };
    checks.add(one);
    return () => {
      checks.delete(one);
    };
  }, [checks, check, element, validate]);
  const left = useCallback(
    () => (shown === undefined && value !== undefined && value === first ? true : check()),
    [shown, value, first, check],
  );
  return { said: error ?? following, left, check };
}

// ---------------------------------------------------------------------------------------------
// Focus and caret kept across a redraw
// ---------------------------------------------------------------------------------------------

/** The caret of a kept field that was taken out of the window while it had the keyboard. */
const kept = new Map<string, { start: number | null; end: number | null }>();

/**
 * **The keyboard and the caret stay in a filter that is drawn again** (R168, R14): a field with
 * `keepsFocus` that leaves the window while it holds the keyboard hands its caret on, and the
 * field drawn in its place in the same pass takes them back. React keeps the keyboard in a box it
 * keeps; this is for a box it replaces. What is not taken back by the next task is forgotten, so
 * a field drawn later never takes the keyboard from where the person has gone.
 */
function useKept(key: string | undefined): RefCallback<HTMLInputElement> {
  return useCallback(
    (element: HTMLInputElement | null) => {
      if (!key || !element) return;
      const caret = kept.get(key);
      if (caret) {
        kept.delete(key);
        // The same box drawn again with a new ref still has them.
        if (element.ownerDocument.activeElement !== element) {
          element.focus({ preventScroll: true });
          if (caret.start !== null && caret.end !== null) {
            try {
              element.setSelectionRange(caret.start, caret.end);
            } catch {
              // A kind of box with no caret to place (an email box): the keyboard is enough.
            }
          }
        }
      }
      return () => {
        if (element.ownerDocument.activeElement !== element) return;
        const held = { start: element.selectionStart, end: element.selectionEnd };
        kept.set(key, held);
        setTimeout(() => {
          if (kept.get(key) === held) kept.delete(key);
        });
      };
    },
    [key],
  );
}

/** One ref from several. A callback that gives no cleanup back is handed `null` when the element
 *  goes, as React hands it to a ref of its own (a roving group's, an anchor's). */
function joined<T>(...refs: (Ref<T> | undefined)[]): RefCallback<T> {
  return (value: T | null) => {
    const cleanups: (() => void)[] = [];
    for (const ref of refs) {
      if (typeof ref === "function") {
        const cleanup = ref(value);
        cleanups.push(typeof cleanup === "function" ? cleanup : () => ref(null));
      } else if (ref) ref.current = value;
    }
    return () => {
      for (const cleanup of cleanups) cleanup();
      for (const ref of refs) if (ref && typeof ref !== "function") ref.current = null;
    };
  };
}

// ---------------------------------------------------------------------------------------------
// The parts
// ---------------------------------------------------------------------------------------------

/**
 * **The room for why a value is refused** (N26, #3033): a field that can be refused (it has
 * `validate`, or an `error` prop, set or not) keeps one line under its box whether or not it says
 * anything, so a refusal that comes or goes moves nothing around it.
 *
 * - **Heard when it lands.** The line is an alert, as every refusal in the window is, and it is in
 *   the accessibility tree from the start, empty: a live region that is already there when its
 *   words arrive is announced by VoiceOver on WebKit, where one shown and filled at once may not
 *   be. While it says something it describes the box (`aria-describedby`), in full.
 * - **Read whole by sight.** A refusal too long for its line (the core's words in a settings
 *   row) is cut short at rest; while the field holds the keyboard, or the pointer is on the line,
 *   the whole of it is drawn over what follows, from where the line starts, so nothing moves. That
 *   copy is for the eye alone (`aria-hidden`): the line's own words are what is heard. An input
 *   error is the one kind of such content WCAG 1.4.13 does not ask to be dismissable.
 */
function ErrorLine({ id, said }: { id: string; said: string | undefined }) {
  const words = useRef<HTMLSpanElement>(null);
  const [clipped, setClipped] = useState(false);
  useLayoutEffect(() => {
    const line = words.current;
    if (line === null) {
      setClipped(false);
      return;
    }
    // Measured at rest, on the one line, which never changes how it is drawn: the whole copy is
    // another element, so showing it never makes the line look as if it fits.
    const measure = () => setClipped(line.scrollWidth > line.clientWidth);
    measure();
    const watch = new ResizeObserver(measure);
    watch.observe(line);
    return () => watch.disconnect();
  }, [said]);
  return (
    <p
      data-scope="field"
      data-part="error"
      id={id}
      role="alert"
      data-clipped={said !== undefined && clipped ? "" : undefined}
    >
      {said !== undefined && (
        <>
          <span ref={words}>{said}</span>
          {clipped && (
            <span data-whole="" aria-hidden="true">
              {said}
            </span>
          )}
        </>
      )}
    </p>
  );
}

/** Whether a field can be refused, and so keeps its error's line. */
function refusable(props: { validate?: unknown; error?: unknown }): boolean {
  return props.validate !== undefined || "error" in props;
}

/** The ids that describe the box: the call's own, and the error's while it says something. */
function describedBy(own: string | undefined, error: string, said: string | undefined) {
  return [own, said !== undefined ? error : undefined].filter(Boolean).join(" ") || undefined;
}

/**
 * **A disabled field's hint is its description for good**, as a disabled button's is (#2908): a
 * disabled box leaves the Tab order, so its tooltip is the pointer's alone, and the reason it is
 * off would otherwise never be heard. Hands back the call's props described by a hidden node with
 * the hint, and that node, to draw inside the field.
 */
function useReason<
  P extends { isDisabled?: boolean; tooltip?: string; "aria-describedby"?: string },
>(props: P): { described: P; reason: ReactNode } {
  const id = `${useId()}-reason`;
  const tip = props.isDisabled === true ? props.tooltip : undefined;
  if (tip === undefined || tip === "") return { described: props, reason: null };
  return {
    described: {
      ...props,
      "aria-describedby": [props["aria-describedby"], id].filter(Boolean).join(" "),
    },
    reason: (
      <span id={id} hidden>
        {tip}
      </span>
    ),
  };
}

/** The field's label, help and error around its control. */
function Around({
  label,
  help,
  error,
  children,
}: {
  label?: ReactNode;
  help?: ReactNode;
  error?: ReactNode;
  children: ReactNode;
}) {
  return (
    <>
      {label !== undefined && (
        <Label data-scope="field" data-part="label">
          {label}
        </Label>
      )}
      {children}
      {help !== undefined && (
        <Text slot="description" elementType="p" data-scope="field" data-part="help">
          {help}
        </Text>
      )}
      {error}
    </>
  );
}

/** A press on the box but off the words (its padding, its glass) puts the caret in it. */
function intoTheBox(event: MouseEvent<HTMLElement>) {
  const box = event.currentTarget.querySelector<HTMLElement>("input, textarea");
  if (!box || event.target === box) return;
  event.preventDefault();
  box.focus();
}

/** The props React Aria's `TextField` takes from a call. */
function fieldProps<E extends HTMLElement>(
  props: Common<E>,
  said: string | undefined,
  errorId: string,
) {
  return {
    value: props.value,
    onChange: props.onChange,
    id: props.id,
    "aria-label": props["aria-label"],
    "aria-labelledby": props["aria-labelledby"],
    "aria-describedby": describedBy(props["aria-describedby"], errorId, said),
    isDisabled: props.isDisabled,
    isReadOnly: props.isReadOnly,
    isRequired: props.isRequired,
    isInvalid: said !== undefined || props.isInvalid === true,
    validationBehavior: "aria" as const,
    autoFocus: props.autoFocus,
    name: props.name,
    maxLength: props.maxLength,
    autoComplete: props.autoComplete ?? "off",
    inputMode: props.inputMode,
    enterKeyHint: props.enterKeyHint,
    className: props.className,
    style: props.style,
    "data-scope": "field",
    "data-part": "root",
  };
}

/** The `data-*` attributes a call hands the element typed in. */
function dataOf(props: object): Record<string, unknown> {
  return Object.fromEntries(Object.entries(props).filter(([key]) => key.startsWith("data-")));
}

/** The native attributes and handlers that go on the element typed in. */
function elementProps<E extends HTMLElement>(props: Common<E>, left: () => boolean) {
  return {
    ...dataOf(props),
    tabIndex: props.tabIndex,
    onMouseDown: props.onMouseDown,
    placeholder: props.placeholder,
    spellCheck: props.spellCheck ?? false,
    role: props.role,
    "aria-expanded": props["aria-expanded"],
    "aria-controls": props["aria-controls"],
    "aria-activedescendant": props["aria-activedescendant"],
    "aria-autocomplete": props["aria-autocomplete"],
    onKeyDown: props.onKeyDown,
    onKeyUp: props.onKeyUp,
    onFocus: props.onFocus,
    onBlur: (event: FocusEvent<E>) => {
      left();
      props.onBlur?.(event);
    },
    onPaste: props.onPaste,
    onSelect: props.onSelect,
    onClick: props.onClick,
    onPointerDown: props.onPointerDown,
    onCompositionStart: props.onCompositionStart,
    onCompositionEnd: props.onCompositionEnd,
  };
}

/** **A field for one line**: a name, a token, an address, a filter. */
export function TextField(props: TextFieldProps) {
  const { described, reason } = useReason(props as OneLine & { type?: string });
  const one = described;
  const { type = "text", size = "md", icon, columns, list, keepsFocus, inputRef, ref } = one;
  const box = useRef<HTMLInputElement>(null);
  const element = useCallback(() => box.current, []);
  const { said, left } = useInline(one.value, one.validate, one.error, element);
  const keep = useKept(keepsFocus);
  const refs = useMemo(() => joined(box, keep, inputRef, ref), [keep, inputRef, ref]);
  const own = useId();
  const errorId = `${own}-error`;
  const [focusedWithin, setFocusedWithin] = useState(false);
  const [hovered, setHovered] = useState(false);
  const hasIcon = icon !== undefined && icon !== null;
  const glass = hasIcon && (
    <span data-scope="text-input" data-part="icon" aria-hidden="true">
      {icon}
    </span>
  );
  const error = refusable(one) ? <ErrorLine id={errorId} said={said} /> : null;
  if (type === "password" || one.value === undefined) {
    // **A box that holds its own words** (a secret's value): a native input the family draws, so
    // the words are never in React's state, nor in the page's markup, as React Aria's own state
    // would put them. Tied to its label, help and error by id, and marked with the states its
    // parts are keyed on, as React Aria marks them.
    const id = one.id ?? `${own}-box`;
    const described =
      [
        one["aria-describedby"],
        one.help !== undefined ? `${own}-help` : undefined,
        said !== undefined ? errorId : undefined,
      ]
        .filter(Boolean)
        .join(" ") || undefined;
    const invalid = said !== undefined || one.isInvalid === true;
    const disabled = one.isDisabled ? "true" : undefined;
    const changed = (event: ChangeEvent<HTMLInputElement>) => {
      const words = event.currentTarget.value;
      if (type === "password")
        (props.onChange as ((filled: boolean) => void) | undefined)?.(words !== "");
      else one.onChange?.(words);
    };
    return (
      <div
        data-scope="field"
        data-part="root"
        data-disabled={disabled}
        className={one.className}
        style={one.style}
      >
        {one.label !== undefined && (
          <label data-scope="field" data-part="label" htmlFor={id}>
            {one.label}
          </label>
        )}
        <div
          data-scope="text-input"
          data-part="root"
          data-size={size}
          data-icon={hasIcon ? "" : undefined}
          data-hovered={hovered && !one.isDisabled ? "true" : undefined}
          data-focus-within={focusedWithin ? "true" : undefined}
          data-disabled={disabled}
          data-invalid={invalid ? "true" : undefined}
          onMouseDown={intoTheBox}
          onPointerEnter={() => setHovered(true)}
          onPointerLeave={() => setHovered(false)}
          onFocus={() => setFocusedWithin(true)}
          onBlur={() => setFocusedWithin(false)}
        >
          {glass}
          <Tooltip tip={one.tooltip}>
            <input
              {...elementProps(one, left)}
              ref={refs}
              id={id}
              type={type}
              name={one.name}
              maxLength={one.maxLength}
              autoComplete={one.autoComplete ?? "off"}
              autoFocus={one.autoFocus}
              inputMode={one.inputMode}
              enterKeyHint={one.enterKeyHint}
              disabled={one.isDisabled}
              readOnly={one.isReadOnly}
              required={one.isRequired}
              size={columns}
              list={list}
              aria-label={one["aria-label"]}
              aria-labelledby={one["aria-labelledby"]}
              aria-describedby={described}
              aria-invalid={invalid || undefined}
              onChange={changed}
            />
          </Tooltip>
        </div>
        {one.help !== undefined && (
          <p data-scope="field" data-part="help" id={`${own}-help`}>
            {one.help}
          </p>
        )}
        {error}
        {reason}
      </div>
    );
  }
  return (
    <AriaTextField {...fieldProps(one, said, errorId)} type={type}>
      <Around label={one.label} help={one.help} error={error}>
        <Group
          data-scope="text-input"
          data-part="root"
          data-size={size}
          data-icon={hasIcon ? "" : undefined}
          onMouseDown={intoTheBox}
        >
          {glass}
          <Tooltip tip={one.tooltip}>
            <Input {...elementProps(one, left)} ref={refs} size={columns} list={list} />
          </Tooltip>
        </Group>
        {reason}
      </Around>
    </AriaTextField>
  );
}

/** **A field for lines**: an answer, one entry per line. */
export function TextArea(given: TextAreaProps) {
  const { described: props, reason } = useReason(given);
  const { rows, grows = false, inputRef, ref } = props;
  const box = useRef<HTMLTextAreaElement>(null);
  const element = useCallback(() => box.current, []);
  const { said, left } = useInline(props.value, props.validate, props.error, element);
  const refs = useMemo(() => joined(box, inputRef, ref), [inputRef, ref]);
  const errorId = `${useId()}-error`;
  const error = refusable(props) ? <ErrorLine id={errorId} said={said} /> : null;
  return (
    <AriaTextField {...fieldProps(props, said, errorId)}>
      <Around label={props.label} help={props.help} error={error}>
        <Group
          data-scope="textarea"
          data-part="root"
          data-grows={grows ? "" : undefined}
          onMouseDown={intoTheBox}
        >
          <Tooltip tip={props.tooltip}>
            <AriaTextArea {...elementProps(props, left)} ref={refs} rows={rows} />
          </Tooltip>
        </Group>
        {reason}
      </Around>
    </AriaTextField>
  );
}
