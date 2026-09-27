# Roadmap and release status

This is a short guide to the [full rollout plan](../ROLLOUT_PLAN.md), not a delivery schedule.
Status reviewed September 27, 2026. Published [GitHub Releases](https://github.com/Seckcey/plenipo/releases)
are the authority for available installers; merged source and acceptance reports describe development.

| State                      | Work                                                                                                                                                                         | Evidence                                                                                                                                             |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Published through v1.6.0   | Organization and delegation, model routing, permissions and approvals, local activity history, browser/desktop controls, lessons, five AI tool adapters, and SSH server work | [v1.6.0 release and limits](https://github.com/Seckcey/plenipo/releases/tag/v1.6.0), [earlier releases](https://github.com/Seckcey/plenipo/releases) |
| Merged development, v1.7.0 | Shared visual system, light/dark frame, gallery, and updated page presentation                                                                                               | [Phase 12A acceptance report](phases/phase-12a-acceptance-report.md)                                                                                 |
| Planned                    | Free/Pro editions and license activation                                                                                                                                     | [Edition plan](editions.md), [rollout plan](../ROLLOUT_PLAN.md)                                                                                      |
| Planned                    | Sales department on HubSpot                                                                                                                                                  | [Sales decision](adr/ADR-018-sales-on-hubspot-no-paperclip.md)                                                                                       |
| Accepted plan; not built   | Embedded terminal panel                                                                                                                                                      | [Terminal panel decision](adr/ADR-031-terminal-panel.md), [Phase 12 checklist](phases/phase-12-checklist.md)                                         |

Not every automated check is a real-world acceptance test. For example, the v1.6.0 notes retain a
pending Windows check against a real SSH server. Read the relevant [phase checklist and acceptance
report](phases/) for what was tested and what remains.

Suggest priorities in [Discussions](https://github.com/Seckcey/plenipo/discussions). For a new
feature, agree on scope before building it; see [CONTRIBUTING.md](../CONTRIBUTING.md).
