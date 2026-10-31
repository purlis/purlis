### Changed

- **An update's review says when it brings along a skill under a built-in skill's name** (Skill studio preview, #3728). Before, such an update was refused only once accepted. Now its review says "This update brings along secrets, whose name belongs to a built-in skill, so it can't be taken until the source renames it.", and Accept update stays unavailable with that reason. `purlis skill update` marks the skill where it lists what comes along, prints the line, and asks nothing.
