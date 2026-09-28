# Website decision: a React Flow sample

Accepted by the owner on September 28, 2026, after the homepage interaction research.

## Decision

Use a website-only React Flow island for the sample workforce. The owner explicitly selected
React Flow, replacing the research proposal's lighter native-module recommendation. Keep the
existing static site and start loading the island when the page opens. The owner's September 28
follow-up replaces the original click-to-open entry with automatic loading. Do not extract
desktop components or connect to desktop commands, providers, accounts, or saved work.

## Reason

The sample needs movable cards, connected reporting lines, pan/zoom, a minimap, and keyboard
support. React Flow provides those diagram controls; custom cards and panels follow the
owner's current Plenipo screenshots. Authored examples make the demo useful without exposing
private screenshots or implying that the website can run the desktop application.

## Tradeoffs and checks

React and React Flow add more JavaScript than the earlier 80 KB native-module target. Keep
that difference explicit, measure actual transfer and interaction costs, and preserve a static
HTML example plus the list alternative. The diagram bundle now loads on normal page entry;
there is no click, scroll, or idle gate. This moves its network/startup cost onto initial load.
Reserve space while loading, avoid focus or scroll changes on automatic mount, honor reduced motion, and test
both keyboard and touch-oriented paths. Build-compression guards are 180,000 bytes of JS and
12,000 bytes of CSS; measured browser evidence belongs in website acceptance.

Keep readable content during loading and on failure. Failed imports can be retried. A loading/ready
guard prevents duplicate roots, and returning to the static example stays static until the visitor
explicitly reopens it. Only that explicit action may focus the sample, and only if focus has not
already moved elsewhere while it loaded.

The existing standalone website Docker context requires its own npm lock. The monorepo keeps
its pnpm lock for workspace checks. Validate both frozen-install paths and keep website
dependencies in its package; the root application package is outside this work item's scope.

See [website operations](../development/website.md) and
[website acceptance](../development/website-validation.md).
