# Workers see the CAPTCHA they try — Checklist

**Status:** built and tested with a real Chromium on Linux against a synthetic check laid out
like reCAPTCHA's; the owner's Windows check is below. **ADR-032 (workers see the CAPTCHA they
try, and hear how each try went)** was accepted by the owner on 2026-09-27 ("improve the
anti-captcha success rate"). It amends **ADR-029 (workers try a CAPTCHA three times before
handing it to the owner)**.

## Why the tries did not work before

- [x] Found: a real check's checkbox lives in its own frame, and the page reader never listed
      frames, so a worker had nothing to click
- [x] Found: the page helper drew Plenipo's sign inside every frame, and in the check's small
      frame the sign's pill covered the checkbox
- [x] Found: a worker could not tell a passed check from an open one, and a refused click
      counted as a try

## Plenipo's browser (`crates/capabilities/src/browser/`)

- [x] `page.js`: a check's widget frame is listed as a control named as its checkbox, with its
      maker (reCAPTCHA, hCaptcha, Cloudflare Turnstile, Arkose); a badge that works by itself
      (invisible reCAPTCHA) is not
- [x] `page.js`: a click on the widget goes to where its checkbox sits, not the frame's middle
- [x] `page.js`: `captchaState` says whether a check shows, is passed (the maker's answer field
      holds a value), opened a puzzle, or is only a badge; `read` carries it as `captchaInfo`
- [x] `page.js`: the sign is drawn only in the top page
- [x] `tab.rs`: `captcha_verdict` watches the check for up to 4 seconds after a try; a passed or
      gone check starts the tries over; the pointer glides to a control and holds the button a
      moment; an "owner input" report while Plenipo acts is ignored
- [x] `operate.rs`: the try is counted once the click or key press happened; the result says
      "That was try N of 3" and how the check stands; the page notice names the checkbox, a
      passed check, a puzzle, or an invisible badge; a click, key, or hand-off aimed at a passed
      check is refused with "passed already"

## Tests

- [x] The synthetic website's `/captcha-frame`: a 304 × 78 frame with a 28-pixel checkbox at
      the left, which writes the answer into the page's `g-recaptcha-response` field when
      clicked; `?puzzle` opens a tall puzzle frame instead
- [x] End to end (`a_captcha_in_its_own_frame_is_clicked_and_its_verdict_read`): the page read
      names the checkbox; the click lands on it and the result says the check passed; a further
      click is refused; no approval and nothing sent; the puzzle variant points at the hand-off
- [x] The three earlier CAPTCHA tests still pass (three counted tries, the hand-off, the switch
      off)
- [x] All pre-push checks from `CLAUDE.md`

## Owner's check on Windows (about 5 minutes)

1. Install the build from this PR's **Windows** check (or the next release).
2. Give a Web Assistant a task on a website of yours that shows a reCAPTCHA or Turnstile
   checkbox (a form of your own is best; check the site's terms first).
3. In the Activity trail, the page read should list a control like
   `checkbox "I'm not a robot (reCAPTCHA)" (the CAPTCHA's own checkbox: clicking it is one
try)`, and the click's result should say **That was try 1 of 3** and then either **The check
   is passed** or **The check now shows a puzzle**.
4. When it says a puzzle opened, the worker should hand the check to you
   (`browser_person_check`): Plenipo's browser comes to the front with the purple sign, you solve
   it, press **Approve**, and the worker continues.
5. Inside the check's frame there should be **no** Plenipo border or pill; the sign frames the
   whole page only.
