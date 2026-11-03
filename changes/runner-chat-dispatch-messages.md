### Fixed

- A chat on a pool's runner can ask, note, tell and answer through dispatch again. What it sends is queued on the runner, and the runner now sends it on to the app itself, once and in order, including whatever was queued while the runner's link was down, once the link is back. Before, every such message stayed on the runner and was never delivered.
