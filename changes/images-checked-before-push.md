### Changed

- **The runner and cluster operator images are checked before they are pushed.** Each
  architecture's image is built once into a file, checked, and only then pushed by its digest,
  so an image that fails its checks never reaches the registry. The `main` tag moves last,
  only after the image has been pulled and checked with no sign-in.
