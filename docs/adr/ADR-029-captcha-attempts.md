# ADR-029: Workers try a CAPTCHA three times before handing it to the owner

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** ADR-020 (Plenipo's browser and computer use), section 5; ADR-023 (on/off switches
  in Settings), sections 4 and 5 and its rejected alternative

## Context

ADR-020 section 5 refused every worker touch of a CAPTCHA, and ADR-023 section 4 built the
hand-off (`browser_person_check`): the page comes to the front, the owner solves the check, and
the worker waits. ADR-023 considered "let workers attempt CAPTCHAs (up to three tries)" and
rejected it: trying a check that a person is present gets past a website's security, which
breaks most sites' terms, can break computer-misuse laws, and can get the owner's accounts
banned.

The owner now asks for those tries after all: a worker that hits a CAPTCHA should try to answer
it **up to 3 times** before the check is handed over. The owner accepts the risk for their own
accounts and websites, and Settings keeps warning about website terms. The risks ADR-023 listed
stay real; this decision bounds them rather than removing them:

- Tries are few and counted per page (3), so a worker cannot grind at a check.
- After the tries, the hand-off to the owner is unchanged: the owner is in the loop before the
  worker gives up, and the approval card says how many tries the worker used.
- The **Hand me checks that a person is using a website** switch off means the old behavior at
  once: no tries, and the worker stops and says so.
- What never changes: no solving services, no working around a check (no fetching the page a
  different way to dodge it), and no typing secrets.

## Decision

1. **Three counted tries.** While the hand-off switch is on, a worker may work on a CAPTCHA:
   clicking its control, typing, choosing, and moving around inside it all go ahead. **One try is
   one submitted answer**: clicking the CAPTCHA's own control (its checkbox or box) or pressing
   Enter or Space in it. Everything else is part of a try and does not count on its own. The
   result of each submitted answer tells the worker "That was try N of 3". Tries are counted on
   the page's tab; when a page read shows the CAPTCHA is gone the count starts over, and it also
   starts over after an owner hand-off. Once the tries are used up, every action in the CAPTCHA
   is refused.
2. **Refusal after the third try.** The next action aimed at the CAPTCHA is refused with "you
   have tried it 3 times … call browser_person_check". The worker may also hand over earlier.
3. **The hand-off is unchanged** (`browser_person_check`, ADR-023 section 4). Its approval card
   now says the worker tried the check N times and could not get past it (or that the worker did
   not try it). Worker instructions, role templates, and tool descriptions say "try up to 3
   times, then hand it to the owner" instead of "never try".
4. **Switch off = no tries.** With **Hand me checks that a person is using a website** off, a
   CAPTCHA stops the worker at once, as in ADR-020: clicks, types, and key presses in it are
   refused, and the worker is told to stop and say so.
5. **Counted, recorded, visible.** Tries are ordinary tool calls: they go through Guard and the
   website lists as any click would, they appear in the Activity trail with their screenshots,
   and data the page itself sends after them is held for the owner as usual. The check's own
   provider answers inside its embedded frame, which the network gate does not hold.
6. **Settings copy says so.** The switch's hint and the websites notice say a worker tries a
   CAPTCHA at most 3 times before handing it over, and the "always on" note says workers never
   try one more than 3 times.

## Consequences

- A simple check (a checkbox that passes on its own) is usually cleared without waking the
  owner; work no longer stops dead at every CAPTCHA.
- Website-terms risk now includes the worker's own tries. Settings keeps its "check a website's
  terms" notice, and the switch hint tells the owner workers try.
- The browser tests that asserted "clicking a CAPTCHA is refused" now assert three counted tries
  and then refusal, plus a full tries-then-hand-off test.

## Alternatives considered

- **Keep ADR-023's rejection (no tries).** Rejected by the owner: work stops dead at every
  CAPTCHA, and the owner asked for tries knowing the risks ADR-023 recorded.
- **A separate switch or a number setting for the tries.** Rejected for now: the hand-off switch
  already gates the behavior (off = no tries). A setting can come later if the owner wants one.
- **Uncounted retries.** Rejected: an uncounted retry loop is exactly the grinding that gets
  accounts banned.
