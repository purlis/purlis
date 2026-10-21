### Changed

- **The cluster operator's downloadable install names its image by digest.** Each published
  operator image's run artifact holds `install/`: the plain manifests and the Helm chart of the
  same commit, with the image and the chart's default digest already set to it. The kind
  end-to-end tests install exactly that (#2281).
