# Built-in skills' trigger tests, until they ship beside them

A built-in skill in `plugin/skills/` is its `SKILL.md` alone (#3686,
`skills::tests::each_built_in_skill_is_its_skill_md_alone`), because Make my own copy copies that
one file. So a built-in's trigger tests wait here, in the `evals/` folder they will have, outside
the bundle the app ships.

Spec #5714 lets a built-in keep its `evals/` beside `SKILL.md`. Once that rule is on `main`, move
each `<skill>/evals/` here into `plugin/skills/<skill>/` and delete this folder.

- `purlis-report/evals/`: 24 real purlis bugs and 24 that are not, at the strict bar, with where
  each came from (#5699).
