# ADR-257: Catch made-up answers, step 2 — links, sending work back, and repeat failures

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Supervisors and up detect
  hallucinations and act accordingly."; item 4.8 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md)). One part, "stop", is built
  differently from the plan's words; see **Stopping a worker** below. It is for the owner to
  accept at review.
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.8)
- **Number:** in the 250s with Phase 25's other later decisions (see ADR-250).
- **Builds on:**
  [ADR-256 (check answers against what really happened)](ADR-256-check-answers-against-what-really-happened.md),
  step 1.

> **On screen** (ADR-010, plain words and rank names): "Doesn't match the record: names a link
> that doesn't exist: https://…" or "names a pull request that doesn't exist: …"; in the Ledger,
> "Sent back by Website Supervisor: Run the tests you said passed" and "Received as a handoff,
> work sent back to fix"; a notice, "Senior Developer's answers keep not matching the record"; a
> worker's Experience, "57 — 4 lessons you kept, 17 tasks done; 3 answers didn't match the
> record".

## In short

Step 1 (ADR-256) checks an answer against Plenipo's own record. Step 2 adds three things:

1. **Links and pull requests named in an answer are checked.** A link no step on the record
   shows is looked at, at most three per answer. One that doesn't exist is a mismatch like the
   others: the answer goes back once, then up marked.
2. **A lead can send work back.** When a reply doesn't hold up, the lead hands the same task back
   to the worker who did it, with what to fix. Plenipo records it on both tasks.
3. **You hear when a worker's answers keep failing.** On the third answer sent back in a week,
   and again on the tenth, a notice. Its Experience says how many of its answers didn't match.

**Accepting this record means** accepting the three above, and that "stop" is built as below.

## Decision

### Links

- **Only where the answer can be trusted.** A pull request on GitHub is asked of GitHub's own
  `gh`, signed in as you, the way Plenipo's GitHub tools reach GitHub (ADR-016). Any other link is
  visited only when its website is on **your allowed websites** (Settings → Websites), through
  Guard: one request, over https (plain http only for an address on this computer or your local
  network that the list names), never with a user name or password, no redirect followed, and
  nothing read from the answer but its status.
- **"Doesn't exist" is only "not found".** A 404 or 410. Any other answer means it exists, or that
  Plenipo can't tell. GitHub's and GitLab's pages are never visited this way: they say "not found"
  for a private page, which would look like a made-up link.
- **Not checked is not a mismatch.** A website not on your list, no `gh`, or no answer: the link
  is left alone. A link a step on the record already shows (a pull request Plenipo opened, a page
  a worker opened) is not looked at again, and a link looked at in the last ten minutes is not
  looked at twice.

### Sending work back

- **How a lead says it.** In its handoff request, `"sendBack": "<task ID>"`, with what to fix as
  the objective, to the worker who did it. The replies message tells leads how.
- **What Plenipo checks.** The task is one the lead itself handed on, it is finished, and the
  request goes to the same worker (the same position, or the same AI tool). Anything else is
  refused with the reason.
- **Workers never control each other.** Liaison gives the work back as a new request, with the
  earlier task as its context, counted like every other handoff. It is recorded on the earlier
  task (`liaison.work_sent_back`, with who sent it back and why) and on the new one
  (`sentBack`).

### Stopping a worker

The plan says "supervisors can stop a worker". A lead's own turn **waits** while its team works
(ADR-008), and it hears from them only when all its requests are answered, so it never sees a
worker still working. There is nothing for it to stop. What a lead can do instead is what this
record adds: decline an answer and send the work back. Stopping a worker at once stays **yours**,
with Stop on every worker and Stop all (items 3.3 and 3.4). If you want leads to stop their team
mid-work, that changes how leads wait (ADR-008) and is its own decision.

### Repeat failures

- **A notice** (Problems) on a worker's third answer sent back within seven days, and again on its
  tenth: "Senior Developer's answers keep not matching the record. 3 of its answers this week
  didn't match what Plenipo saw it do, and were sent back. Check its work, or give it another AI
  model."
- **Its Experience** counts every answer of its sent back, from the Ledger (nothing new is
  stored).

## Consequences

- **Easier:** a made-up link is caught the same way as a made-up test run, and a lead can act on
  a bad reply without starting over.
- **Harder:** a link check costs one request to a website you allowed, or one `gh` call, at most
  three per answer.
- **Watch for:** a website on your allowed list that says "not found" for pages you must sign in
  to see. Take it off the list, or tell the worker to name a public page.

## Alternatives considered

- **Visit every link.** Rejected: workers' answers would make Plenipo reach any address on the
  internet. Your allowed websites are the line Guard already draws for workers.
- **Let leads cancel a worker's running task.** Not possible while leads wait for their team
  (above); kept for you.
- **A notice on every answer sent back.** Rejected: one is normal. Three in a week is a pattern.
