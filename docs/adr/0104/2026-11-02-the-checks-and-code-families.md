# Amendment, 2026-11-02 (#4481): the checks and code families

Pool setup (spec #4469) needs a list of things checked, each with its verdict, and text to copy.
The prototype on `prototype/pool-setup` hand-drew both, and the person ruled that they become kit
parts as they are needed, with stories as the record.

- **The checks family** (`app/src/ui/checks/`): `CheckList` and `CheckLine`. A line's variant is its
  verdict: `ok`, `missing`, `running`, `todo`, `warn` and `skip`, each told apart by its mark and
  its words as well as its colour (WCAG 1.4.1). Its `data-scope` by hand is `purlis/checks-family`,
  an error.
- **The code family** (`app/src/ui/code/`): `CodeBlock`, text in a box with its Copy button, on the
  button family. The text takes the keyboard, so it can be scrolled there. Its `data-scope` by hand
  is `purlis/code-family`, an error.
