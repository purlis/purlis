#!/usr/bin/env python3
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

"""Regenerate `crates/unicode/src/tables.rs` from CPython's own `unicodedata` and the UCD.

Every Unicode table the core carries is generated here, because the core has exactly one
Unicode question per table and two hand-written answers to one question is how the two
implementations come to render one line differently.

Eleven tables. Four answer three questions charter's Python asks, two are UCD properties,
three are emoji data, and two are the letters the joiner rule names:

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

* Bidi_Control and Default_Ignorable_Code_Point on their own — which of those reorder the
  text around them, and which take no room? Only to say what KIND of invisible character a
  review found; the INVISIBLE table decides whether it is one.

* Extended_Pictographic, Emoji_Modifier, and the keycap and RGI tag sequences, from the emoji
  data (#3219) — is this selector, joiner or tag part of an emoji
  (`purlis_unicode::hidden`)? The first two say which characters a presentation selector or
  a ZERO WIDTH JOINER may follow, so VS16 after `©`, `▶` or `㊗` reads as text where
  hand-written block ranges missed them. The sequences are the emoji whose invisible
  characters are their own spelling: a keycap (`#`, VS16, U+20E3) and the flags of England,
  Scotland and Wales. Only the RGI tag
  sequences: a tag run after a black flag that no platform draws as a flag is drawn as the
  black flag alone, the tags hidden behind it, which is how text is smuggled behind an emoji.

* Alphabetic, in the Brahmic blocks (Devanagari to Malayalam, U+0900-U+0D7F) and in the
  Arabic script's blocks and presentation forms (#3470) — is the character beside a joiner a
  letter it may join (`purlis_unicode::hidden`)? Rust's `char::is_alphabetic` and the
  window's `\\p{Alphabetic}` each answer from their own Unicode version, which need not be this
  file's; both now ask these two tables, which the window reads from `bindings.ts`.

The Rust port must answer the first four the same way for every codepoint, and the only source
that cannot drift from Python is Python. So the ranges are *generated from the oracle* rather
than taken from a crate with its own opinion (`unicode-width` deviates deliberately: it widens
emoji presentation sequences and zeroes default-ignorables, neither of which
`east_asian_width` says).

**The inputs are pinned.** The four UCD files are committed under `tools/ucd/<version>/`
(the copies `https://www.unicode.org/Public/<version>/` publishes, byte for byte, under
`tools/ucd/LICENSE`), and the header of `tables.rs` records each one's sha256. Each must say
the version `unicodedata` is of, or the script stops: two Unicode versions in one file is the
drift the generator exists to prevent. Run it with a CPython whose Unicode version is the one
the header names (CPython 3.14 for 16.0.0; the recorded answers in `tests/fixtures/recorded/`
were taken under it, ADR 0046):

    python3.14 tools/gen-unicode-tables.py > crates/unicode/src/tables.rs
    rustfmt --edition 2024 crates/unicode/src/tables.rs

`--ucd <folder>` reads the files from elsewhere; by default they are `tools/ucd/<version>/`
beside this script. The `rustfmt` is not optional: CI checks formatting over every file,
generated or not, and this one prints four ranges to a line where `rustfmt` wants one.

**CI checks it.** A step of the `rust (fmt, clippy)` job in `.github/workflows/ci.yml` runs
these two commands under the pinned CPython and fails on any difference from the committed
`tables.rs`: a hand edit, a changed input or a generator change without its regenerated
output each turn the job red. Moving to a new Unicode version is three changes in one pull
request: the new folder of files, the step's Python, and the regenerated `tables.rs`.
"""

import argparse
import hashlib
import os
import re
import sys
import unicodedata


def sha256(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def read(folder, file, version_line):
    """The lines of `file` in `folder`, after checking it is of `unicodedata`'s Unicode version.

    `version_line` is the line, exactly, that names the version in that file's header: the
    first line of a UCD file, a `Version` line of an emoji file.
    """
    path = os.path.join(folder, file)
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    if version_line not in (line.strip() for line in lines[:12]):
        sys.exit(f"{path} is not of Unicode {unicodedata.unidata_version}: "
                 f"no {version_line!r} in its header")
    return lines


def fields(line):
    """A data line's `;`-separated fields, or None for a comment or a blank line."""
    body = line.split("#", 1)[0].strip()
    return [part.strip() for part in body.split(";")] if body else None


def codepoints(path, cps):
    """`XXXX` or `XXXX..YYYY` as a range of codepoints."""
    m = re.fullmatch(r"([0-9A-F]{4,6})(?:\.\.([0-9A-F]{4,6}))?", cps)
    if m is None:
        sys.exit(f"{path}: cannot read {cps!r}")
    return range(int(m.group(1), 16), int(m.group(2) or m.group(1), 16) + 1)


def ucd_property(folder, file, lines, prop):
    """The codepoints a UCD property file (`lines` of `file`) gives `prop`, as a set."""
    out = set()
    for line in lines:
        row = fields(line)
        if row is not None and row[1] == prop:
            out.update(codepoints(os.path.join(folder, file), row[0]))
    if not out:
        sys.exit(f"{os.path.join(folder, file)} gives no codepoint {prop}")
    return out


def emoji_sequences(folder, file, lines, kinds):
    """Each sequence `emoji-sequences.txt` (`lines` of `file`) lists as one of `kinds`."""
    found = {kind: [] for kind in kinds}
    for line in lines:
        row = fields(line)
        if row is not None and row[1] in found:
            found[row[1]].append([int(cp, 16) for cp in row[0].split()])
    for kind, seqs in found.items():
        if not seqs:
            sys.exit(f"{os.path.join(folder, file)} lists no {kind}")
    return [seq for seqs in found.values() for seq in seqs]


VERSION = unicodedata.unidata_version
EMOJI_VERSION = ".".join(VERSION.split(".")[:2])
args = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
args.add_argument(
    "--ucd",
    default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "ucd", VERSION),
    help="the folder holding the four UCD files (default: tools/ucd/<version>/)",
)
args = args.parse_args()
# Each input, the line in its header that names its version, and its lines.
INPUTS = {
    "DerivedCoreProperties.txt": f"# DerivedCoreProperties-{VERSION}.txt",
    "PropList.txt": f"# PropList-{VERSION}.txt",
    "emoji-data.txt": f"# Used with Emoji Version {EMOJI_VERSION} and subsequent minor revisions (if any)",
    "emoji-sequences.txt": f"# Version: {EMOJI_VERSION}",
}
if not os.path.isdir(args.ucd):
    sys.exit(f"no {args.ucd}: this CPython's unicodedata is of Unicode {VERSION}, and no UCD "
             "files of that version are committed. Run the CPython the header of tables.rs names.")
lines = {file: read(args.ucd, file, want) for file, want in INPUTS.items()}
ignorable = ucd_property(args.ucd, "DerivedCoreProperties.txt", lines["DerivedCoreProperties.txt"],
                         "Default_Ignorable_Code_Point")
bidi = ucd_property(args.ucd, "PropList.txt", lines["PropList.txt"], "Bidi_Control")
alphabetic = ucd_property(args.ucd, "DerivedCoreProperties.txt", lines["DerivedCoreProperties.txt"],
                          "Alphabetic")
pictographic = ucd_property(args.ucd, "emoji-data.txt", lines["emoji-data.txt"], "Extended_Pictographic")
modifier = ucd_property(args.ucd, "emoji-data.txt", lines["emoji-data.txt"], "Emoji_Modifier")
sequences = emoji_sequences(args.ucd, "emoji-sequences.txt", lines["emoji-sequences.txt"],
                            ("Emoji_Keycap_Sequence", "RGI_Emoji_Tag_Sequence"))


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
default_ignorable = ranges(lambda c: ord(c) in ignorable, first=0)
nonprintable = ranges(lambda c: not c.isprintable(), first=0)



def fmt_sequences(name, seqs):
    lines = [f"pub static {name}: [&[char]; {len(seqs)}] = ["]
    for seq in seqs:
        lines.append("    &[" + ", ".join(f"'\\u{{{cp:X}}}'" for cp in seq) + "],")
    lines.append("];")
    return "\n".join(lines)


extended_pictographic = ranges(lambda c: ord(c) in pictographic, first=0)
emoji_modifier = ranges(lambda c: ord(c) in modifier, first=0)
BRAHMIC_BLOCKS = [(0x0900, 0x0D7F)]
ARABIC_BLOCKS = [(0x0600, 0x06FF), (0x0750, 0x077F), (0x08A0, 0x08FF), (0xFB50, 0xFDFF),
                 (0xFE70, 0xFEFF)]


def letters_in(blocks):
    return ranges(lambda c: ord(c) in alphabetic and any(lo <= ord(c) <= hi for lo, hi in blocks),
                  first=0)


brahmic_letter = letters_in(BRAHMIC_BLOCKS)
arabic_letter = letters_in(ARABIC_BLOCKS)

print(f"//! Generated by `tools/gen-unicode-tables.py` from Unicode {VERSION}: CPython's `unicodedata`")
print(f"//! and these files of `tools/ucd/{VERSION}/`, by their sha256:")
print("//!")
for file in INPUTS:
    print(f"//! - `{file}` {sha256(os.path.join(args.ucd, file))}")
print("//!")
print("//! Do not hand-edit: regenerate, so the tables and the oracle stay one answer. CI regenerates")
print("//! this file and fails on any difference (a step of the `rust (fmt, clippy)` job).")
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
print("/// Codepoints with the Default_Ignorable_Code_Point property: they draw as nothing.")
print(fmt("DEFAULT_IGNORABLE", default_ignorable))
print()
print("/// Codepoints `str.isprintable()` is false for, which is what `repr()` escapes:")
print("/// `Cc`, `Cf`, `Cs`, `Co`, `Cn`, `Zl`, `Zp` and `Zs` other than the space.")
print(fmt("NONPRINTABLE", nonprintable))
print()
print("/// Codepoints with the Extended_Pictographic property: the emoji, and the codepoints kept")
print("/// for them, that a presentation selector or a ZERO WIDTH JOINER may follow.")
print(fmt("EXTENDED_PICTOGRAPHIC", extended_pictographic))
print()
print("/// Codepoints with the Emoji_Modifier property: the skin tones, which a ZERO WIDTH JOINER")
print("/// may follow.")
print(fmt("EMOJI_MODIFIER", emoji_modifier))
print()
print("/// The keycap and RGI tag sequences of `emoji-sequences.txt`: the emoji whose selector or")
print("/// tags are their own spelling, so each of their characters reads as text.")
print(fmt_sequences("EMOJI_SEQUENCES", sequences))
print()
print("/// Codepoints with the Alphabetic property in the Brahmic blocks, Devanagari to Malayalam")
print("/// (U+0900-U+0D7F): the letters either joiner may stand between there.")
print(fmt("BRAHMIC_LETTER", brahmic_letter))
print()
print("/// Codepoints with the Alphabetic property in the Arabic script's blocks and presentation")
print("/// forms: the letters a ZERO WIDTH NON-JOINER may stand between, as Persian writes it.")
print(fmt("ARABIC_LETTER", arabic_letter))
print()
print(f"wide={len(wide)} combining={len(comb)} invisible={len(invisible)} "
      f"bidi_control={len(bidi_control)} default_ignorable={len(default_ignorable)} "
      f"nonprintable={len(nonprintable)} extended_pictographic={len(extended_pictographic)} "
      f"emoji_modifier={len(emoji_modifier)} emoji_sequences={len(sequences)} "
      f"brahmic_letter={len(brahmic_letter)} arabic_letter={len(arabic_letter)}", file=sys.stderr)
