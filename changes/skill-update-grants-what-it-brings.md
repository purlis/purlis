### Changed

- **An update of a project skill reaches where the skill does** (Skill studio preview, #4186). A
  skill the update brings along lands with the updated skill's grant, instead of at the root
  alone. A project skill its newer commit refers to is granted to those workspaces too, in the
  same commit, as an install with that grant does. The update review says both before you accept
  it, and an update whose skill was regranted since its review changes nothing.
