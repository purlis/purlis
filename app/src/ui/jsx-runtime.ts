// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **React's JSX runtime, with each part's classes** (#4351): the window's JSX is compiled against
 * this module (`jsxImportSource` in `vite.config.ts`), which hands React the props a part carries
 * with `ds-<scope>` and `dp-<part>` added beside its attributes (`partClasses.ts`). Everything else
 * is React's own.
 */
import { Fragment, jsx as reactJsx, jsxs as reactJsxs } from "react/jsx-runtime";
import { parted } from "./partClasses";

export { Fragment };

export const jsx: typeof reactJsx = (type, props, key) => reactJsx(type, parted(props), key);

export const jsxs: typeof reactJsxs = (type, props, key) => reactJsxs(type, parted(props), key);
