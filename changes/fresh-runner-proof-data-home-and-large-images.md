### Fixed

- A task's own cluster runner also stays while the task saved a file in purlis's data home on it outside every repository, such as beside the projects. purlis's own folders on the runner are left out wherever the image puts them, and a runner on a large image is no longer kept for its size: only the places its user may write are looked through at each check (#5028, #5029, #5030).
