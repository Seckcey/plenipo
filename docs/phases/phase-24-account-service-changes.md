# Changes for the account service — Community follows its switch

**From:** Plenipo, Phase 24 (Community), part 24C. Plenipo is made by 8 West Ventures, LLC.
**To:** a separate session in the account service's own repository (`Seckcey/plenipo-account`,
private), under its own rules (ADR-101, ADR-102).
**Status:** Asked, 2026-10-02. Plenipo's builder does not change that repository; this list is what
it needs. Tick each item when it is merged there.

**Why:** [ADR-170 (Community follows 8 West's switch)](../adr/ADR-170-community-follows-8-wests-switch.md)
and [ADR-171 (switching Community on part by part)](../adr/ADR-171-switching-on-part-by-part.md). The
written rules are in `contracts/community/v1` (README §1, the `Open` schema entry, and the examples
`open.json`, `error-not-open.json`, and `error-update-needed.json`), from the pull request that
changes the contract.

> This file holds no address, key, or sign-in for the service, and must never hold one.

## In short

Plenipo shows **Coming soon** while the service says Community is not open, and starts working the
moment the owner switches Community on, with no new Plenipo release. For that, the service must say
"not open" in its own way, be able to turn away an old version of Plenipo, and have a switch for
linked organizations and one for collaborators.

## The changes

- [ ] **Copy the new contract.** Pin `community/v1/` again from the commit that changes it, as
      `contract/PINNED.md` describes, and run the contract tests.
- [ ] **"Not open" has its own answer.** While Community is switched off, every `/v1/community/*`
      request answers `503` with `{"error": "not_open", "message": "Community isn't open yet."}`
      instead of `unavailable`, still reading nothing. `unavailable` stays for "busy, or something
      went wrong".
- [ ] **"Is Community open?"** `GET /v1/community/open`: no pass, no body, nothing kept. It answers
      `200` with `Open` (`{"links": <bool>, "collaborators": <bool>}`) when Community is open, and
      `not_open` when it is off. Limit: 60 an hour per internet address (`too_many`).
- [ ] **The lowest version of Plenipo** allowed in Community, set by 8 West (for example
      `COMMUNITY_MIN_APP_VERSION`; unset means no lowest version). While it is set, every
      `/v1/community/*` request, `open` and the two sign-in requests included, whose `User-Agent` is
      not `Plenipo/<major>.<minor>.<patch>` at or above it answers `403` with
      `{"error": "update_needed", "message": "Update Plenipo to use Community."}`. Versions compare as
      numbers, part by part.
- [ ] **The order of checks:** first whether Community is open (`not_open`, reading nothing), then the
      version (`update_needed`), then everything as today.
- [ ] **A switch for linked organizations** (for example `COMMUNITY_LINKS_ENABLED`), off by default.
      While it is off: every path in contract §9 answers `not_open`; so do items of kinds
      `link_note`, `objective`, `objective_state`, and `answer`, and thanks `for` `link_answer`; and
      the service makes no `link_*` notices. `open` answers `"links": false`.
- [ ] **A switch for collaborators** (for example `COMMUNITY_COLLABORATORS_ENABLED`), off by default.
      While it is off: every path in contract §10, items of kind `collab_note`, and thanks `for`
      `collaborator` answer `not_open`; and the service makes no `collab_*` notices. `open` answers
      `"collaborators": false`.
- [ ] **Tests** for each of the above, including: nothing is read or kept while Community is off; an
      old version is refused before its pass is looked at; a closed part refuses its items while
      messages keep working.
- [ ] **The owner's switches,** written in the service's own instructions: Community, then linked
      organizations, then collaborators, each only after its security review passes (ADR-171), and
      the lowest version raised to the release each review passed.

## Not asked

- Nothing about GIFs: the service keeps answering `gifs_unavailable` until the owner chooses the GIF
  library (ADR-164 §4).
- Nothing about the free month of Pro for invitations: it is still to come in the service (ADR-169
  §6).
