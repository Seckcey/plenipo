# Publish checked website changes automatically

Status: Accepted for implementation by the owner on September 30, 2026.

Plenipo is made by 8 West Ventures, LLC. Its Coastline website already checks GitHub
on a timer, as described in
[the decision to follow published releases](ADR-069-website-follows-releases.md).
That updater notices new installers and corrected release notes, but website-only
changes such as terms and privacy pages need a manual `--force` run.

The owner asked to automate website updates while retaining the existing private
Coastline SSH connection. Extend the installed updater rather than add credentials
or a GitHub Actions runner on Coastline.

The updater compares the Git content identifiers for `apps/website` and the Website
workflow at the selected source and the running site's `sourceRevision`. It rebuilds
when those files, the published release, or its notes change. Unrelated app changes
leave the site alone. An old or unavailable running source cannot establish that the
website is unchanged, so it plans a checked rebuild.

Before building, the latest CI run must succeed for the exact selected source commit.
The latest Website run must succeed for the most recent first-parent commit that
changed the website files or its workflow. This allows a notes-only correction to
reuse checks for identical website files. Missing, pending, failed, cancelled, or
wrong-commit results defer the update. An API failure stops the run. `--force` only
overrides the unchanged-site decision; it does not override the checks.

The selected version remains a published release with a working Windows installer.
The existing immutable exports, deployment locks, container checks, history, and
rollback remain. The checks also verify the terms and privacy pages if that source
includes them. A missing policy or wrong page identity rolls back the update.
Container lookup uses the app's Compose labels so unattended runs can find the
previous image without depending on the service's working directory.

The timer uses outgoing public GitHub requests and its existing local Docker access.
GitHub receives no SSH key, Tailscale login, or permission to send server commands.
The updater is not self-updating: a maintainer must install this script once using
[the upgrade steps](../development/website.md#upgrade-an-existing-updater).

The cloud session used for implementation has no laptop connection, Tailscale setup,
or Coastline credentials. Repository publication is separate from installing the
script, enabling the timer, and verifying the public policies.
