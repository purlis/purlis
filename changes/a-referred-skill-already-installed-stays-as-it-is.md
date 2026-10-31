### Fixed

- **Installing a skill that refers to one installed already goes ahead** (Skill studio preview,
  #4032). A skill another one refers to (`skills:<name>`) is no longer brought along into a level
  that holds it already, where the install used to refuse with "already have it installed".
  The review says the skill it refers to is installed there already and is left as it is, and
  reads again when you pick another level. The same holds for Install all… and
  `purlis skill install`.
