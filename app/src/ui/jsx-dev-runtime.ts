// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** React's development JSX runtime, with each part's classes, as `jsx-runtime.ts` is. */
import { Fragment, jsxDEV as reactJsxDev } from "react/jsx-dev-runtime";
import { parted } from "./partClasses";

export { Fragment };

export const jsxDEV: typeof reactJsxDev = (type, props, key, isStatic, source, self) =>
  reactJsxDev(type, parted(props), key, isStatic, source, self);
