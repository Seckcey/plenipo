# Homepage automatic entry correction

The owner asked for the working demo when the page opens. The initial production release
`155ad42` deliberately placed import and mount work inside the start-button handler. This
correction calls the same loader immediately on entry. It adds no click, scroll, visibility,
or idle gate. Network transfer still takes time; readable content remains while loading.

## Source and behavior

Implementation `e08b0ac4f3214c542689dd7f6f95b2e155bdb6c2` is based on main `567550f5`.
The only runtime change is `apps/website/main.js`: a static/loading/ready guard prevents duplicate
starts, automatic readiness does not focus a control, and returning to static does not trigger
another automatic load. Explicit retry/reopening may focus the demo only if focus remains on
that button. The loading button uses `aria-disabled` with the state guard, preserving native
keyboard focus until completion. Focus ownership is captured before hiding its fallback ancestor.
Failed imports retain their fresh retry URL and readable fallback.

The coordinator caught an earlier real-browser regression: a native disabled button lost focus,
and hiding its ancestor before checking focus also invalidated ownership. The corrected regression
harness models both blur cases; native browser reactivation is a separate acceptance requirement.

Five tests run the actual page script with browser/module I/O substituted: immediate load without
input, duplicate-load protection, static persistence/reopening, import failure/retry, focus moved
away during a slow retry, and render failure recovery. These lifecycle checks complement real
browser testing; they do not claim to simulate layout or browser import caching.

## Browser evidence

On isolated Coastline preview source `35781ba`, fresh Chromium navigation at 1440 px and 390 px
created one interactive root without any click or scroll. Desktop had six cards; phone selected
the List view. Focus remained `BODY` and scroll position remained zero on fresh navigation.
Phone width stayed within the viewport. The coordinator separately reproduced automatic entry
at 1280 × 720, then checked the supervisor's details.

Conversation switching and sample decision/activity still work. Returning to static left zero
interactive roots, stayed static through unrelated workflow navigation, and could be reopened.
Blocking the actual hashed module on a fresh navigation showed readable failure content and an
enabled retry; restoring the resource and retrying mounted one root. With scripts disabled,
the static sample, two download links, and native expandable disclosure remained usable. Reduced
motion produced zero active animations. Ordinary runs had no warning/error entries; intentional
blocked-resource checks are separate. No provider or application requests were observed.

The automation's no-JavaScript locator click depends on page scripting and timed out; native
focus plus Enter verified the disclosure instead. A normal reload can restore the browser's
previous scroll position; fresh-navigation checks above measure whether automatic mounting moves it.

## Initial-page cost

One cache-disabled Chromium run used a 390 × 844 viewport, 4× CPU slowdown, 1.6 Mbps download,
and 150 ms latency. The observer was installed 629.7 ms after navigation, before the demo existed;
resource times and buffered performance entries are relative to navigation.

| Observation                             | Result                         |
| --------------------------------------- | ------------------------------ |
| Demo stylesheet request / response end  | 609.6 / 933.8 ms               |
| Demo module request / response end      | 610.3 / 2,539.5 ms             |
| Mounted root, following animation frame | 2,574.1 ms                     |
| Compressed module / stylesheet bodies   | 156,360 / 6,291 bytes          |
| Recorded long task                      | 145 ms, starting at 3,449.5 ms |
| Recorded non-input layout shift         | 0.0223788                      |
| Focus / scroll at mount                 | `BODY` / 0                     |

This is one emulated browser observation, not a speed guarantee or a zero-shift/50 ms pass.
The later long task belongs to the full page observation, not solely to module startup. Automatic
entry moves the existing bundle cost onto navigation. It does not make network transfer instant.

## Repository checks

The full repository frontend checks at `35781ba` passed: 304 UI, 263 desktop, 10 website,
and 13 script tests, plus version, formatting, lint, documentation links, and type checks.
Rust formatting, strict workspace clippy, and all 1,068 workspace tests passed with no failures
or ignored tests. Regenerated bindings matched all 241 existing files byte for byte. Rust
completed at 2026-09-28 13:42:29 UTC. The later focus correction changes only website JavaScript
and its lifecycle tests; its affected frontend/build/browser/parity rerun is recorded below.

## Release scope

Main after `155ad42` added build-time installer version templating. The correction release must
build the reviewed merged revision with `PLENIPO_VERSION=1.6.0`. Compare every generated file
against the accepted `155ad42` website with only the corrected loader substituted. Permit only
the intended loader/hash-reference and release identity changes; record any demonstrated template
whitespace separately. Unexplained output differences block release. No installer repair,
desktop feature, external PR #88, provider connection, or customer data is included.

Production remains on `155ad42` until the correction is merged, deployed, and verified. Retain
that image/source/configuration as rollback. The initial `35781ba` comparison verified 62 files:
61 byte-identical, with only `index.html` template whitespace different. An initial extraction hit
read-only directory permissions; the retained failure was resolved by extracting tar-stream file
bytes into private writable directories. No application change was needed for extraction.

The focus-corrected source archive SHA-256 is
`3bbf381f30df666f43010c2f841c277b32e97aee352e37b8026a3fd7d1d68236`.
At `e08b0ac`, the full frontend check repeated successfully with the same test totals, and the
standalone frozen npm install/image build passed. The repeated output comparison again verified
all 62 files: 61 exact byte matches and only `index.html` whitespace different, with no unexpected
differences. The preview image is
`sha256:99ffbc7dcb07bd77e8f19c696b0fa436ad568b25eb04d6637980bfb8692d041e`.

Native Enter after static return now focuses `demo-tab-team`. During a throttled import retry,
moving to the outside `feature-models` control preserved focus there after mount. Fresh desktop
and phone navigation again mounted one root without focus/scroll changes, with phone List selected
and no horizontal overflow. The coordinator independently reproduced fresh entry and explicit
reopening on this exact source. Browser/network/viewport overrides were restored. One initial
slow-import wait timed out before network completion; the completed state and repeated outside-focus
check confirmed correct behavior. Original timing evidence remains labeled `35781ba`; the focus
correction does not change the island bundle or its automatic entry behavior.

Production identity is recorded after release; this implementation receipt is not a release claim.
