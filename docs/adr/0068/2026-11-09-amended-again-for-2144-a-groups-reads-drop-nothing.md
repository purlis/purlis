# Amended again for #2144, a group's reads drop nothing, 2026-11-09

The amendment for #2144 keeps what a group of blanked boxes holds together out of every later
answer. This note changes its item 2 (D-2144-2) and widens its item 1; a field's reads stay as
the amendment for #2311 has them, and nothing else changes.

1. **A group keeps every value it held.** A read of a group of boxes, from the boxes themselves or
   from the page's tree, keeps each joined value it reads and drops none of the group's earlier
   ones, however short. Which elements are a group is read from the page: a short blanked field
   beside the boxes, with no label between, joins it, so a later read can hold the code with
   that field's text after it, and the code itself has to stay kept. More is kept out, never
   less. This supersedes the part of D-2144-2 that has a group's read drop its earlier joined
   values shorter than 8 characters. *D-2144-3, decided in implementation, to be confirmed.*
2. **A separator is any of the ones a page puts in a code.** What a group holds is found with any
   spaces, dashes, dots, slashes, middle dots or bullets between its characters, where its
   item 1 names spaces and dashes. *D-2144-4, decided in implementation, to be confirmed.*
