#!/usr/bin/env python3
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

"""Regenerate `crates/unicode/src/tables.rs` from CPython's own `unicodedata` and the UCD.

Every Unicode table the core carries is generated here, because the core has exactly one
Unicode question per table and two hand-written answers to one question is how the two
implementations come to render one line differently.

Five tables. Four answer three questions charter's Python asks, and one property list:

* `charter/tui.py:_char_width` — how many cells does this character claim?

      if ch < "\\x80":                 return 1
      if unicodedata.combining(ch):   return 0
      return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1

* `charter/contain.py:one_line` — has this character no glyph, so that printing it could
  restructure a line of a report?

      unicodedata.category(ch) in {"Cc", "Cf", "Cs", "Zl", "Zp"} or (ch.isspace() and ch != " ")

  and, since SK-49 (#3172), purlis's one rule for a character a reader does not see adds
  Unicode's Default_Ignorable_Code_Point to it: the variation selectors, the Hangul fillers,
  the combining grapheme joiner and the whole tag plane, which draw as nothing and which
  `unicodedata` cannot name. The review, the terminal tables, the hook and `one_line` all ask
  this one table (`purlis_unicode::invisible`).

* `repr()` — would Python escape this character rather than print it?

      not ch.isprintable()

* Bidi_Control — which of those reorder the text around them? Only to say what KIND of
  invisible character a review found; the table above decides whether it is one.

The Rust port must answer each of them the same way for every codepoint, and the only source
that cannot drift from Python is Python. So the ranges are *generated from the oracle* rather
than taken from a crate with its own opinion (`unicode-width` deviates deliberately: it widens
emoji presentation sequences and zeroes default-ignorables, neither of which
`east_asian_width` says).

Run it with a CPython whose Unicode version is the one the header of `tables.rs` names (the
recorded answers in `tests/fixtures/recorded/` were taken under it, ADR 0046), and with `--ucd`
naming a folder that holds that same version's `DerivedCoreProperties.txt` and `PropList.txt`
(from `https://www.unicode.org/Public/<version>/ucd/`). The two files must say the version
`unicodedata` is of, or the script stops: two Unicode versions in one file is the drift the
generator exists to prevent. Paste the output into `crates/unicode/src/tables.rs`. The header line records which Unicode
version the tables are of, so a future reader can tell whether a disagreement is a bug or a
version. It is a generator for a checked-in file, run by hand: nothing in CI or in a build runs
it.

    python3 tools/gen-unicode-tables.py --ucd <folder> > crates/unicode/src/tables.rs
    rustfmt --edition 2024 crates/unicode/src/tables.rs

The `rustfmt` is not optional: CI checks formatting over every file, generated or not, and
this one prints four ranges to a line where `rustfmt` wants one.
"""

import argparse
import os
import re
import sys
import unicodedata


def ucd_property(folder, file, prop):
    """The codepoints `file` (a UCD property file in `folder`) gives `prop`, as a set.

    Stops unless the file's first line names the Unicode version `unicodedata` is of.
    """
    path = os.path.join(folder, file)
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    stem = file.removesuffix(".txt")
    want = f"# {stem}-{unicodedata.unidata_version}.txt"
    if not lines or lines[0].strip() != want:
        sys.exit(f"{path} is not of Unicode {unicodedata.unidata_version}: it starts {lines[:1]}")
    out = set()
    for line in lines:
        body = line.split("#", 1)[0].strip()
        if not body:
            continue
        cps, name = (part.strip() for part in body.split(";", 1))
        if name != prop:
            continue
        m = re.fullmatch(r"([0-9A-F]{4,6})(?:\.\.([0-9A-F]{4,6}))?", cps)
        if m is None:
            sys.exit(f"{path}: cannot read {cps!r}")
        lo = int(m.group(1), 16)
        hi = int(m.group(2) or m.group(1), 16)
        out.update(range(lo, hi + 1))
    if not out:
        sys.exit(f"{path} gives no codepoint {prop}")
    return out


args = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
args.add_argument("--ucd", required=True, help="a folder holding DerivedCoreProperties.txt and PropList.txt")
args = args.parse_args()
ignorable = ucd_property(args.ucd, "DerivedCoreProperties.txt", "Default_Ignorable_Code_Point")
bidi = ucd_property(args.ucd, "PropList.txt", "Bidi_Control")


def ranges(pred, first=0x80):
    """`pred` as inclusive ranges, from `first` up.

    The surrogates are never asked: `chr(0xD800)` is a `str` in CPython and is not a `char`
    in Rust at all, so whatever Python says about one is unreachable from the port. They are
    reported as False and so split a range, which is why WIDE and COMBINING — which start
    above them — read the same as before this note was written.
    """
    out, start, prev = [], None, None
    for cp in range(first, 0x110000):
        ok = False if 0xD800 <= cp <= 0xDFFF else pred(chr(cp))
        if ok:
            if start is None:
                start = cp
            prev = cp
        elif start is not None:
            out.append((start, prev))
            start = None
    if start is not None:
        out.append((start, prev))
    return out


def fmt(name, rs):
    lines = [f"pub static {name}: [(u32, u32); {len(rs)}] = ["]
    row = []
    for a, b in rs:
        row.append(f"(0x{a:04X}, 0x{b:04X}),")
        if len(row) == 4:
            lines.append("    " + " ".join(row))
            row = []
    if row:
        lines.append("    " + " ".join(row))
    lines.append("];")
    return "\n".join(lines)


wide = ranges(lambda c: unicodedata.east_asian_width(c) in ("W", "F"))
comb = ranges(lambda c: unicodedata.combining(c) != 0)
# From 0, not from 0x80: these two decide about the ASCII controls, which are the characters
# that can actually forge a line, and about DEL.
invisible = ranges(
    lambda c: unicodedata.category(c) in {"Cc", "Cf", "Cs", "Zl", "Zp"}
    or (c.isspace() and c != " ")
    or ord(c) in ignorable,
    first=0,
)
bidi_control = ranges(lambda c: ord(c) in bidi, first=0)
nonprintable = ranges(lambda c: not c.isprintable(), first=0)

print(f"//! Generated by `tools/gen-unicode-tables.py` from Unicode {unicodedata.unidata_version}.")
print("//!")
print("//! Do not hand-edit: regenerate, so the tables and the oracle stay one answer.")
print()
print("/// Codepoints whose `east_asian_width` is `W` or `F` — two terminal cells.")
print(fmt("WIDE", wide))
print()
print("/// Codepoints with a non-zero canonical combining class — zero terminal cells.")
print(fmt("COMBINING", comb))
print()
print("/// Codepoints a reader does not see, so purlis marks them: general category `Cc`, `Cf`,")
print("/// `Cs`, `Zl` or `Zp`, `str.isspace()` and not the space, or Default_Ignorable_Code_Point.")
print(fmt("INVISIBLE", invisible))
print()
print("/// Codepoints with the Bidi_Control property: they reorder the text around them.")
print(fmt("BIDI_CONTROL", bidi_control))
print()
print("/// Codepoints `str.isprintable()` is false for, which is what `repr()` escapes:")
print("/// `Cc`, `Cf`, `Cs`, `Co`, `Cn`, `Zl`, `Zp` and `Zs` other than the space.")
print(fmt("NONPRINTABLE", nonprintable))
print()
print(f"wide={len(wide)} combining={len(comb)} invisible={len(invisible)} "
      f"bidi_control={len(bidi_control)} nonprintable={len(nonprintable)}", file=sys.stderr)
