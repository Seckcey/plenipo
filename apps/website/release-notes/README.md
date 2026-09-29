# Release notes for the website build

The automatic website update (`../deploy/auto-release.sh`) copies the shown release's notes here as
`vX.Y.Z.md` before it builds the image, because a container build sees only `apps/website`. Keep
this folder empty in the repository apart from this file. A build without notes for its version
shows a link to the release on GitHub instead.
