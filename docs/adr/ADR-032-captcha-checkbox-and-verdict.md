# ADR-032: Workers see the CAPTCHA they try, and hear how each try went

- **Status:** Accepted (by the owner, 2026-09-27: "improve the anti-captcha success rate")
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** ADR-029 (workers try a CAPTCHA three times before handing it to the owner);
  ADR-020 (Plenipo's browser and computer use), section 5

## Context

ADR-029 lets a worker try a CAPTCHA (a check that a person is using a website) up to 3 times
before handing it to the owner. In practice the tries almost never worked, for three reasons
found in the code, not in the AI tools:

1. **The worker could not reach the checkbox.** Real checks (reCAPTCHA, hCaptcha, Cloudflare
   Turnstile) keep their checkbox inside their own frame (an `iframe` from the check's maker).
   The page reader listed only the page's own controls, never a frame, so a worker saw "This
   page shows a CAPTCHA" and had nothing to click. The test website's check was a plain button
   on the page itself, so the tests passed anyway.
2. **Plenipo's own sign covered the checkbox.** The page helper runs in every frame of a page,
   and each one drew the sign (the colored border and the "… is using this browser" pill). In a
   check's small frame (304 × 78 pixels) the pill lay right over the checkbox and took the
   click.
3. **The worker could not tell how a try went.** After a click the page still "showed a
   CAPTCHA" (the widget stays on the page after it is passed), so the worker either clicked
   again, burning tries, or handed over a check that was already passed. A denied click also
   counted as a try, since tries were counted before the click happened.

## Decision

1. **The check's checkbox is a control.** The page reader lists a check's widget frame as a
   control (`e7: checkbox "I'm not a robot (reCAPTCHA)" (the CAPTCHA's own checkbox: clicking
it is one try)`), and the page notice names it ("Its checkbox is e7: click it once, and the
   result says whether the check passed"). A click on it goes to where the checkbox sits in the
   widget (28 pixels in from the left, centered, for the normal reCAPTCHA, hCaptcha, and
   Turnstile widgets; near the top center of a compact one), not to the frame's middle. A
   check's badge that works by itself (invisible reCAPTCHA) is not a control; the page says so.
2. **The sign is drawn only in the top page.** Frames inside a page get no sign of their own.
   The top page's sign covers them anyway.
3. **Every try gets a verdict.** After a counted try (a click on the check's control, or Enter
   or Space in it), Plenipo watches the check for up to 4 seconds and tells the worker how it
   stands, in the same result: **passed** (the check's maker wrote its answer into the page,
   read from the page's answer field: `g-recaptcha-response`, `h-captcha-response`,
   `cf-turnstile-response`, `fc-token`), **a puzzle opened** (a tall frame appeared: take a
   screenshot, or hand it to the owner rather than use up tries), **gone**, or **still there and
   not passed** (read the page again before another try). Each result also says how many tries
   are left.
4. **Passed means done.** When the check is passed or gone, the tries start over (ADR-029
   already did this for a gone check). A click, Enter, or Space aimed at a passed check is
   refused with "passed already: go on with the page", and so is a hand-off
   (`browser_person_check`) for a passed check. The page notice says "passed already" too.
5. **A try counts when it happens.** The try is counted after the click or key press went
   ahead, not when it was prepared, so a click the owner refuses is no try.
6. **A click looks like a click.** The pointer glides to the control in a dozen small steps
   along a gently bowed path and the button is held a moment, for every click a worker makes.
   Plenipo still announces itself as automated (`--enable-automation`, the browser's own bar,
   `navigator.webdriver`); nothing hides that, and no solving service is ever used.
7. **Plenipo's own input is never the owner's.** While Plenipo acts, an "owner input" report
   from a page helper (which a click inside a frame now produces, since the helper in that frame
   cannot see that Plenipo is acting) is ignored. The Take over button and the owner's own
   clicks outside an action still take over as before.

## Consequences

- A checkbox-only check can now actually be passed by a worker: the checkbox is listed,
  reachable, uncovered, and clicked where it is, and the worker is told at once when it passed.
  Picture puzzles still go to the owner: Plenipo cannot list a puzzle's pieces as controls, and
  a coordinate click inside the check's frame would bypass the sensitive-action check that reads
  every control before a click (rejected below).
- Fewer wasted tries: no clicks on a passed check, no hand-off for a passed check, no try
  counted for a refused click.
- The owner's clicks inside a frame while a worker is between actions still count as taking
  over. The owner's clicks inside a frame during the worker's own action (a moment) do not;
  the Take over button and Stop always work.
- What ADR-029 bounded stays bounded: 3 counted tries per page, the same hand-off, the same
  switch (off means no tries), no solving services, no working around a check.
- Tests: the synthetic website gains a check laid out like reCAPTCHA's, with its checkbox in
  its own frame and its answer written into the page (`/captcha-frame`, and `?puzzle` for one
  that opens a puzzle). The test clicks it through the whole stack and checks the verdict, the
  refusal after passing, and the puzzle notice.

## Alternatives considered

- **A coordinate click (`browser_click_at`) so the worker could answer picture puzzles.**
  Rejected: every click today is read from its control first (a Buy button waits for the
  owner, a password field is refused); a click at a bare coordinate would skip that. Puzzles go
  to the owner.
- **Reading inside the check's frame over DevTools (attaching to its out-of-process target) to
  find the exact checkbox.** Not now: it adds a second protocol session per frame for a
  checkbox that sits in a known place in every provider's widget. Worth doing if a provider
  moves its checkbox.
- **Hiding that the browser is automated (`navigator.webdriver`, the automation bar).**
  Rejected: Plenipo's browser is deliberately open about being automated (ADR-020), and hiding
  it is exactly the "working around a check" that ADR-029 rules out.
