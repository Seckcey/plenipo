# ADR-198: The setup tour, with Driver.js

- **Status:** Accepted (the owner, 2026-10-03: "Need a MUCH better tutorial for new organization
  setup and initial sign up that walks the user through setting up AI tools subscriptions, adding
  a department and setting up their first project and then assigning models to the hired tiles.
  Use the Driver.js library."; item 2.9 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted the same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 2 (item 2.9)
- **Amends:** [ADR-030 (one design system for every screen)](ADR-030-design-system.md): one
  outside library, Driver.js, draws the tour's spotlight. [ADR-053 (the organization
  canvas)](ADR-053-the-organization-canvas.md) §21: the canvas's own tour waits while the setup
  tour shows.

> **On screen** (ADR-010, plain words and rank names): "Welcome to Plenipo", then "Sign in to an
> AI tool", "Name your organization, or pick a template", "Add a department", "Set up your first
> project", "Hire the team", "Choose a model for each job", "Give the first objective", and
> "Watch it work". Buttons: **Back**, **Next** (or **Skip this step**, or **Finish**), and ×
> ("Stop the tour for now"). Home and Settings → Organization: **Take the setup tour**, **Pick up
> the setup tour**, or **Take the setup tour again**.

## In short

The only tour was six cards about the canvas, with nothing highlighted. Nothing checked that an AI
tool was signed in, so a new owner could build a whole organization before learning that no
worker could run.

**Accepting this record means:**

1. **A setup tour of nine steps**, in the owner's order: welcome; sign in to an AI tool (it waits
   for a green light); name your organization or pick a template (ADR-197); add a department; set
   up the first project; hire the team, reusing the workers you have (ADR-196); choose a model for
   each job (ADR-195); give the first objective; watch it work.
2. **It moves on only when a step is really done**: a subscription signed in, a department, a
   project, a worker, a model choice changed, an objective given. Steps with nothing to check move
   on with **Next**. Any step can be skipped. A step already done is passed over.
3. **It finds its way:** it opens each step's page by itself, waits for the part of the screen it
   points at (up to about a second and a half, then it shows its words in the middle), and brings
   the first project's supervisor into view on the map.
4. **The page stays usable.** The spotlight dims the rest of the screen, but you can still click,
   scroll, and use the Tab key anywhere, so you can do each step (signing in happens in a tab at
   the bottom). Driver.js blocks all three by default; the tour turns that off.
5. **It pauses while a dialog is open** and comes back when it closes.
6. **It remembers where you stopped, for each organization, on this PC.** It starts by itself in
   a new organization: one with no project yet, and either nothing set up or made in the last
   day. **Take the setup tour** on Home and in Settings → Organization starts it again; after you
   stopped part way, it says **Pick up the setup tour**. If Plenipo closed during the tour, the
   tour comes back where it was.
7. **Colors come from Plenipo's theme**, in light and dark.
8. **Steady marks** (`data-tour`) on the left strip's buttons, + Department, + Project, the Add
   menu, the hire palette, the AI tools list, Settings → Organization, Who uses what, Give an
   objective, Watch, and the New department, New project, Hire, and Set up a Development project
   dialogs.

## Decision

- **Driver.js 1.9 (MIT license)**, about 26 KB, with no dependencies of its own. It draws only the
  tour; every other part on screen still comes from `@plenipo/ui` (the editor and the terminal
  keep their own libraries, as before). Its own stylesheet loads first; Plenipo's styles then set
  its colors, fonts, and buttons from the theme tokens, so ADR-030's "no raw colors" rule still
  holds for Plenipo's code.
- **Security settings stay as they are.** Driver.js sets positions with inline styles, which the
  window's security policy already allows (`style-src-attr 'unsafe-inline'`); it runs no code
  from outside and loads nothing from the internet.
- **A test robot doesn't get the tour by itself.** When WebDriver drives the window (the
  end-to-end tests), the tour starts only from its button.
- The tour's steps and its "done" checks are plain data (`apps/desktop/src/tour/steps.ts`), each
  with a unit test.

## Consequences

- A new owner is walked from an empty organization to a team at work.
- The canvas's six-card tour still shows the first time you open the map, after the setup tour.
- One more outside package to keep up to date.

## Alternatives considered

- **Build the spotlight ourselves.** Rejected: the owner asked for Driver.js, and it already
  handles scrolling, placing the words beside the part, and resizing.
- **Block the rest of the page during the tour** (Driver.js's default). Rejected: signing in
  happens in a tab at the bottom, and every step asks you to do something on the page.
- **One tour for the whole PC.** Rejected: each organization is set up on its own.
