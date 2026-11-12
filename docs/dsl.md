# DSL reference

A file holds one or more `graph` blocks. Indentation sets the structure, and there are no
parentheses. Terms such as *gate* and *sequence edge* are defined in the [glossary](glossary.md).

## Lines

| Line | Meaning |
| --- | --- |
| `graph NAME` | Starts a graph. |
| `node NAME [flags] [@key:value ...]` | A stage. Flags are `loop`, `end`, and any the schema allows. |
| `edge FROM -> TO when COND` | A **gate**: taken when the host sets `COND` true. |
| `edge FROM -> TO` | A **sequence edge**: taken after the host calls `finish()` for the stage. |
| `edge A -> B -> C ...` | An optional chain. See [Chains](#chains). |
| `choose NAME` | A set of `when COND -> TAG` arms, optionally ending in `else -> TAG`. |

## Rules

Graphite checks these when it parses the file. Each violation is an error that points at the
line and column that caused it.

1. Declare all `node` lines before any `edge` or `choose`.
2. The first node is the entry. A run starts there.
3. A node with no outgoing edge is an error unless it is marked `end`. An `end` node may not
   have outgoing edges.
4. A node has at most one sequence edge. When a gate and the sequence edge are both ready, the
   gate wins, whatever the source order.
5. Gates are checked in source order. The first that holds is taken.
6. Cycles are allowed.
7. Tags, annotation keys, and annotation values are opaque strings. The core never branches on
   them.
8. Names must be unique within a graph, and generated file names must not collide.
9. On a node line, flags come before annotations: `node a loop @anim:walk`, never
   `node a @anim:walk loop`.

## Example

A walk cycle with a choice. The full file is [`examples/walk.dsl`](../examples/walk.dsl).

```text
graph walk
  node windup   @anim:windup
  node going    loop @anim:walk @group:locomotion
  node winddown end @anim:winddown
  node stopped  end @anim:idle

  edge windup    -> stopped   when keyup
  edge windup    -> going
  edge going     -> winddown  when keyup

  choose facing
    when left  -> side @flip:true
    when right -> side
    when up    -> up
    else       -> down
```

## Chains

An `edge` line may list several hops with `->`. Each hop is an edge of its own, so a chain is
shorthand for writing the edges out. The one-hop forms work everywhere, and you never need a
chain. The rest of the line applies to every hop:

- `when COND` makes each hop a gate on `COND`.
- Without `when`, each hop is a sequence edge.
- `else T` adds, for each hop, a sequence edge from that hop's source to `T`. Use it for the
  failure path of a stage. The chain's last node gets no fallback, so give it its own line.

Annotations on the line apply to every edge it creates. These lines are equivalent:

```text
edge setup -> baseline when pass
edge setup -> report
edge baseline -> plan when pass
edge baseline -> report

edge setup -> baseline -> plan when pass else report
```

## Flags and annotations

- **Flags** are bare words on a node. `loop` and `end` are always allowed. Domain flags such as
  `retry` must be allowed in an allowlist (see below).
- **Annotations** are `@key:value` pairs. They are passed to the host unchanged. Graphite never
  reads their meaning, so `@anim:walk` and `@task:deploy` mean whatever your host makes of them.

## Allowlists

An allowlist is a TOML file that says which flags and annotation values a graph may use. Pass it
with `--allow` to `check`, `compile`, `run`, or `debug`. Its `[annotations]` table is optional:

```toml
node_flags = ["retry"]

[annotations]
task = ["write", "lint", "fix", "deploy"]
```

Without an allowlist, any annotation is accepted, and only `loop` and `end` are allowed as flags.

With `graphite run --tasks tasks.toml`, the task names are the keys of the manifest's `[tasks]` table.
The allowlist then needs only `node_flags`, and a `task` list in it is replaced by the manifest's keys.
Use one when you want typos in task or flag names to fail at build time. The
[durable workflow example](../examples/workflow/workflow.toml) shows a complete file.

## Conditions and choices

Conditions are set by the host and keep their values until the host changes them. A choice is
evaluated in order: the first arm whose condition holds is selected, and `else` is the fallback.
With no `else`, a choice that matches no arm selects nothing: `choose()` returns `undefined`.
