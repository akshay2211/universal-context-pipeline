# Postgres VACUUM tuning notes

What I've figured out from the last three incidents. Posting here so future-me can search it.

## What VACUUM actually does

Postgres uses MVCC, which means an UPDATE doesn't overwrite a row — it writes a new row version and marks the old one as dead. Those dead row versions accumulate and waste disk + RAM. VACUUM is the garbage collector that reclaims them.

There are two flavors:

- `VACUUM` — reclaims dead tuples but does not return space to the OS. Fast, doesn't lock.
- `VACUUM FULL` — rewrites the table, returns space to the OS, takes an `ACCESS EXCLUSIVE` lock. Effectively offline operation. Don't run on prod tables larger than a few hundred MB without planning.

Autovacuum runs `VACUUM` in the background. The question is whether it runs *often enough* on your table.

## The bloat problem

When autovacuum can't keep up, you get table bloat: the table is logically small but physically huge. Symptoms:

- Sequential scans get slow even though row count hasn't changed.
- Cache hit ratio drops because dead rows take up shared_buffers space.
- Disk usage grows in a way that doesn't track INSERT rate.

The `pgstattuple` extension is the canonical way to measure this:

```sql
SELECT * FROM pgstattuple_approx('big_table');
```

If `dead_tuple_percent` is over ~20% on a hot table, autovacuum is losing the race.

## Tuning knobs that actually matter

In order of how often I've had to touch each:

1. **`autovacuum_vacuum_scale_factor`** — defaults to 0.2 (vacuum when 20% of the table is dead). Too coarse for big tables; a 100M-row table won't get vacuumed until 20M rows are dead. Set per-table to 0.01-0.05 for hot tables.
2. **`autovacuum_naptime`** — how often the autovacuum launcher wakes up. Default 1 minute. Lower it to 15s if you have many hot tables.
3. **`autovacuum_max_workers`** — default 3. If you have many tables that need vacuuming simultaneously, increase to 5-8.
4. **`maintenance_work_mem`** — how much memory each vacuum process gets. Default 64MB is laughably small for big tables. Set to 1-2GB on dedicated DB hosts.

Per-table overrides via `ALTER TABLE ... SET (autovacuum_vacuum_scale_factor = 0.01);` is the right tool for the 80/20 case.

## The XID wraparound thing

The other reason VACUUM matters: Postgres uses a 32-bit transaction ID. Every row carries the XID of the transaction that created it. If a row's XID becomes more than 2 billion older than the current XID, Postgres marks the database read-only to prevent data corruption.

VACUUM updates a "frozen" marker on old rows so they don't count toward wraparound. If autovacuum falls behind on a write-heavy table, you can hit wraparound emergency mode (`age(datfrozenxid) > 2_000_000_000`), and at that point you're doing a manual `VACUUM FREEZE` while the database is read-only. Don't let it get there. Monitor `pg_database.datfrozenxid` and alert at 1.5B.

## What I'd do differently

When we hit the bloat issue in March, my first move was running `VACUUM FULL` during a maintenance window. Worked but was painful. The right answer was per-table autovacuum tuning *before* it became a crisis. Now I include autovacuum config review in the new-table checklist.
