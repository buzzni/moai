# Analyse the tracker with SQL

moai has no query engine inside it, and it does not need one: every row it
knows comes out of `moai show --json`, and a SQL engine that reads JSON can
answer the questions the built-in commands do not. This page uses
[DuckDB](https://duckdb.org), which reads a JSON file as a table with no
setup. Install it on your own machine; moai does not ship it.

Try `moai stats` first. It already counts flow, lead and cycle time, and the
AI work recorded on rows, and it reads the archive too. SQL is for the
questions it does not ask.

## Take a snapshot of the rows

Write the rows to a file once and query the file as often as you like:

    moai show --archived --json > rows.json
    duckdb

`--archived` includes the archive: done rows that have sat in done for a while.
Without it, old closed work is missing from every count. A whole tracker with
bodies runs to megabytes. If you do not need the bodies, keep only the keys you
query with `--fields`:

    moai show --archived --json \
      --fields id,title,kind,status,priority,tags,assignee,created_at,updated_at,status_since,started_at,done_at,due_on,derived_status,derived_epic,derived_milestone,archived \
      > rows.json

Without `work` in `--fields`, moai skips reading the journals, so this file
comes out faster. Add `work` when you query AI work (below).

For a single question, pipe the rows in instead of writing a file:

    moai show --archived --json |
      duckdb -c "SELECT kind, count(*) FROM read_json('/dev/stdin') GROUP BY ALL"

## Know what the columns mean

Each row is one issue, epic, milestone or backlog item (`kind`). The fields are
listed in [Draw the tracker from another program](json.md#read-rows-and-what-they-inherit).
Three things matter for queries:

- **Read membership from the `derived_` keys.** `epic` and `milestone` are what
  the file says. `derived_epic` and `derived_milestone` also hold what a row
  inherits: an issue in an epic stands under that epic's milestone
- **Read a group's column from `derived_status`.** A milestone or an epic has
  no column of its own; its column is read from its members. Use
  `coalesce(derived_status, status)` to get one column for every row
- **A missing key is absent, not `null`.** DuckDB makes a column only for keys
  it sees. If no row in the file has a deadline, there is no `due_on` column,
  and a query that names it fails with a binder error. Name such a column in
  `read_json(..., columns = {...})`, or leave the query out

Times are UTC in ISO 8601 (`2026-11-05T02:22:59Z`). `due_on` and `starts_on` are
calendar days (`2026-11-20`). The queries below cast them explicitly, so they
work whether or not DuckDB has detected the type.

## Count what stands where

Rows by kind and column:

```sql
SELECT kind, coalesce(derived_status, status) AS column_name, count(*) AS n
FROM 'rows.json'
GROUP BY ALL
ORDER BY kind, n DESC;
```

Open work per assignee, ignoring groups and backlog:

```sql
SELECT coalesce(assignee, '(unowned)') AS who, status, count(*) AS n
FROM 'rows.json'
WHERE kind = 'issue' AND status <> 'done'
GROUP BY ALL
ORDER BY who, status;
```

Rows per tag (a row with two tags counts once under each):

```sql
SELECT tag, count(*) AS n
FROM (SELECT unnest(tags) AS tag FROM 'rows.json' WHERE kind = 'issue')
GROUP BY ALL
ORDER BY n DESC;
```

## Measure time

**Lead time per assignee.** The hours from creation to the last move into done,
for closed issues:

```sql
SELECT assignee,
       count(*) AS closed,
       round(median(date_diff('minute', created_at::TIMESTAMPTZ, done_at::TIMESTAMPTZ)) / 60, 1) AS median_hours
FROM 'rows.json'
WHERE kind = 'issue' AND status = 'done' AND done_at IS NOT NULL
GROUP BY ALL
ORDER BY closed DESC;
```

**Cycle time.** The same from `started_at`, when the row first left the first
column. A row with no `started_at` does not say when it started. Leave it out;
do not count it as zero:

```sql
SELECT date_trunc('week', done_at::TIMESTAMPTZ) AS week,
       count(*) AS closed,
       round(median(date_diff('minute', started_at::TIMESTAMPTZ, done_at::TIMESTAMPTZ)) / 60, 1) AS median_hours
FROM 'rows.json'
WHERE kind = 'issue' AND status = 'done' AND started_at IS NOT NULL AND done_at IS NOT NULL
GROUP BY ALL
ORDER BY week;
```

**Work that has sat in its column longest.** `status_since` is when the row
entered the column it stands in now:

```sql
SELECT id, title, status, assignee,
       date_diff('day', status_since::TIMESTAMPTZ, now()) AS days_here
FROM 'rows.json'
WHERE kind = 'issue' AND status NOT IN ('todo', 'done')
ORDER BY days_here DESC
LIMIT 20;
```

**Throughput.** Issues closed per week:

```sql
SELECT date_trunc('week', done_at::TIMESTAMPTZ) AS week, count(*) AS closed
FROM 'rows.json'
WHERE kind = 'issue' AND done_at IS NOT NULL AND status = 'done'
GROUP BY ALL
ORDER BY week;
```

`done_at` stays on a row that was reopened, so the queries above also check
`status = 'done'`.

## Follow deadlines and milestones

Open work past its own deadline:

```sql
SELECT id, title, assignee, due_on::DATE AS due,
       current_date - due_on::DATE AS days_late
FROM 'rows.json'
WHERE status <> 'done' AND due_on IS NOT NULL AND due_on::DATE < current_date
ORDER BY days_late DESC;
```

This reads each row's own `due_on`. A row without one can still follow a
deadline set on its epic or milestone, and `moai ready` orders work by that
followed deadline. To count those rows too, join each row to its
`derived_epic` and `derived_milestone` and take the earliest `due_on` of the
three.

Progress of each milestone, counted over the issues under it:

```sql
WITH m AS (
  SELECT id, title, created_at FROM 'rows.json' WHERE kind = 'milestone'
)
SELECT m.title,
       count(r.id) AS issues,
       count(r.id) FILTER (WHERE r.status = 'done') AS done,
       round(100.0 * count(r.id) FILTER (WHERE r.status = 'done') / nullif(count(r.id), 0)) AS pct
FROM m
LEFT JOIN 'rows.json' AS r ON r.derived_milestone = m.id AND r.kind = 'issue'
GROUP BY m.title, m.created_at
ORDER BY m.created_at DESC;
```

## Count the AI work

Each row carries `work`: one entry per `model:` note, with `provider`, `model`,
`grade`, `why`, `at` and `by`. Keep `work` in `--fields`, or leave
`--fields` out:

    moai show --archived --json --fields id,kind,work > work.json

```sql
SELECT w.provider, w.model, w.grade, count(*) AS lines
FROM (SELECT unnest(work) AS w FROM 'work.json')
GROUP BY ALL
ORDER BY lines DESC;
```

`moai stats --json` gives the same counts under its `work` key. SQL helps when
you want to cut them further, such as by month (`date_trunc('month', w.at::TIMESTAMPTZ)`).

## When it goes wrong

- **`Binder Error: Referenced column ... not found`** — no row in the file has
  that key. See "A missing key is absent", above
- **Old closed work is missing** — the file was written without
  `--archived`, so done rows that have reached archive age are left out
- **The same id appears twice** — a merge can leave two lines for one id.
  Count `DISTINCT id` until the snapshot is resolved
- **Times look shifted** — the values are UTC. Cast to `TIMESTAMPTZ` and set
  DuckDB's time zone (`SET TimeZone = 'Asia/Seoul';`) before you truncate to a day or a week

Decided in: moai-yfv3
