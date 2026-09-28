# Website decision: an optional React Flow sample

Accepted by the owner on September 28, 2026, after the homepage interaction research.

## Decision

Use a website-only React Flow island for the sample workforce. The owner explicitly selected
React Flow, replacing the research proposal's lighter native-module recommendation. Keep the
existing static site and load the island only after the visitor activates it. Do not extract
desktop components or connect to desktop commands, providers, accounts, or saved work.

## Reason

The sample needs movable cards, connected reporting lines, pan/zoom, a minimap, and keyboard
support. React Flow provides those diagram controls; custom cards and panels follow the
owner's current Plenipo screenshots. Authored examples make the demo useful without exposing
private screenshots or implying that the website can run the desktop application.

## Tradeoffs and checks

React and React Flow add more JavaScript than the earlier 80 KB native-module target. Keep
that difference explicit, measure actual transfer and interaction costs, and preserve a static
HTML example plus the list alternative. No diagram bundle should load before activation.
Reserve space during activation, avoid continuous animation, honor reduced motion, and test
both keyboard and touch-oriented paths. Build-compression guards are 180,000 bytes of JS and
12,000 bytes of CSS; measured browser evidence belongs in website acceptance.

The existing standalone website Docker context requires its own npm lock. The monorepo keeps
its pnpm lock for workspace checks. Validate both frozen-install paths and keep website
dependencies in its package; the root application package is outside this work item's scope.

See [website operations](../development/website.md) and
[website acceptance](../development/website-validation.md).
