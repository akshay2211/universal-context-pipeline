# Q3 Planning — meeting notes

Date: 2026-04-09. Attendees: Akshay, Priya, Marcus, Jin.

## Decisions

1. **Ship the migration to Postgres 16 before end of Q3.** We've been on 14 since 2023. Logical replication path is well understood and our staging tests passed. Marcus owns the cutover; targeting first week of August so we have a full month of runway.
2. **Defer the multi-region rollout to Q4.** We agreed it doesn't unblock any Q3 customer, and the compliance review (especially around EU residency) is still in progress. Revisit in October planning.
3. **Cut the legacy admin panel.** Numbers from analytics show <3 active users per week and we've been paying maintenance cost in security review for every release. Priya will announce the deprecation timeline next Monday — 60 day window, full removal by end of June.

## Open questions (carried over)

- **Auth provider consolidation.** We currently run both Auth0 and a homegrown JWT path. Marcus pushing for full Auth0; Jin worried about vendor lock-in cost. Tabled for now — need cost comparison from finance before we can decide.
- **Observability budget.** We're 40% over the Datadog allowance YTD. Options on the table: turn off custom metrics for non-prod, or move to a cheaper logging tier. Priya to draft a one-pager by end of next week.

## Action items

- [Akshay] Write the migration runbook for the Postgres 16 cutover. Due April 23.
- [Marcus] Stand up a staging replica running 16 with the production schema. Due April 16.
- [Priya] Draft and send the admin-panel deprecation note to existing users. Due April 14.
- [Jin] Pull EU data-residency requirements from the compliance team and summarize. Due April 30.

## Themes from the discussion

We spent ~30 minutes on the question of whether to take on the auth provider work in Q3 vs Q4. The consensus was that Q3 is already dense with the Postgres migration and the admin panel deprecation. Marcus made the case that auth is on the critical path for the SOC 2 audit in November, so it can't slip past Q4. Recorded as a hard date.

There was also a strong-but-quiet debate about the team's bandwidth. Jin pointed out we've taken on extra scope in the last two quarters and the engineering retention numbers are starting to slip. We agreed to push back on any new scope additions from product unless they unblock a Q3 commitment.

## Next meeting

Standing weekly check-in stays Tuesdays at 10 a.m. The next planning review is the all-hands on May 7.
