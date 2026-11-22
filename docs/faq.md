# FAQ

## What is Graphite for?

Anything that is a set of states with rules for moving between them, where the rules are easy to
get wrong in plain code: animation states, NPC behavior, and multi-step jobs. Graphite checks the
rules when you build, so a dead end or a misspelled stage is an error with a location, not a bug
you find in play.

## When should I not use it?

- A single `if` or `switch` with two or three branches. The graph adds more than it removes.
- Anything that needs the executor itself to run timers, threads or callbacks. The core has none;
  your host supplies time.
- Production systems. Graphite is an early proof of concept (see the [roadmap](../ROADMAP.md)).

## How is it different from a state-machine library?

Graphite does not run your code. It compiles a description of states and edges into checked data,
then generates typed code for your names, through a front end (TypeScript today). Your host decides when time passes, what a
condition means, and what an animation does. The graph itself is a value you can inspect and test.

## Why is TypeScript the first front end?

Graphite started as a tool for animation state machines in a web game. TypeScript was the obvious
first host, because it could run inside the game without a bridge.

The DSL and the core do not depend on any language. A **front end** generates the code for one host
language, and new front ends can be added without changing the graph language. See
[front ends](front-ends.md) for how to add one.

## Why does `run` need Bun?

`run` and `debug` execute TypeScript, and Bun is the default runtime for it. `npx tsx` also works.
Set `GRAPHITE_JS_RUNNER` to choose another. The generated TypeScript needs no runtime to be
compiled, only to be executed. [`graphite doctor`](cli.md) reports which front ends can run here.

## Do I have to use TypeScript?

No. `check`, `inspect` and the Rust API need no host language at all. `compile` writes the output of
one front end, which is TypeScript today. You need a JavaScript runtime only for `run`, `debug`, and
TypeScript hosts that you write.

## Can a graph have more than one start?

No. The first node is the entry, and a run starts there.

## Can a graph loop forever?

Yes. Cycles are allowed, and a graph with no reachable `end` is valid. The `loop` flag sets a
`looping` field on the node, which your host can read; it changes nothing inside Graphite. Graphite
does not stop a cycle on its own, so your host decides when the loop is done.

## Is it production-ready?

No. The README says so, and [ROADMAP.md](../ROADMAP.md) lists what is missing. The surface may
still change.

## Where do I report a problem?

Open an issue. For a security problem, see [SECURITY.md](../SECURITY.md) instead.

## What do the terms mean?

| Term | Meaning |
| --- | --- |
| **Graph** | A named set of stages and the rules between them. A `.graphite` file holds one or more graphs. |
| **Stage** (node) | One state the graph can be in, such as `idle` or `opening`. The DSL calls it a `node`. |
| **Entry** | The first node in a graph. A run starts there. |
| **End** | A node marked `end`. A run stops there, and the node may have no outgoing edges. |
| **Edge** | A rule that moves the graph from one stage to another. |
| **Gate** | An edge with `when COND`. It is taken when the host sets `COND` to true. |
| **Sequence edge** | An edge without `when`. It is taken after the host calls `finish()` for the stage it leaves. |
| **Condition** | A named true/false value the host sets. It stays set until the host changes it. |
| **Host** | The code that owns time and drives the graph: a game loop, a test, a server, or a durable runner. Graphite never runs by itself. |
| **Step** | One settling of the graph. Every edge that can be taken is taken, in order. |
| **Finish** | The host's report that the current stage's work is done (an animation has played, a job has run). It makes a sequence edge ready. |
| **Choice** | A `choose` block: a set of `when COND -> TAG` arms, with an optional `else`. The host asks which arm holds. |
| **Arm** | One line of a choice. Its tag is returned to the host. |
| **Annotation** | A `@key:value` pair on the `graph` line, a node, an edge or a choice arm, such as `@anim:walk`. Graphite passes it through unchanged. A `default` line adds annotations to every node. |
| **Flag** | A bare word on a node, such as `loop` or `end`. Your schema can allow more. |
| **Tag** | A string an arm returns, such as `side`. Opaque to Graphite. |
| **Allowlist** | A TOML file that lists the flags and annotation values a graph may use, checked when the file is compiled. |
| **Task** | With `run`, the work a node does, named by its `@task:NAME` annotation. |
| **Runner** | The TypeScript file that performs tasks and reports each `pass`, `fail` or `none`. |
| **Outcome** | The result a task reports. `pass` sets the `passed` condition and `fail` sets `failed`. |
| **Checkpoint** | The saved state a durable run writes after each result, so it can resume. |
| **Front end** | The part that turns a checked graph into code for one host language, selected with `--frontend`. See [front ends](front-ends.md). |
| **Machine file** | A file of states, edges and choices, which `run` and `debug` can execute. The default. |
| **Pure file** | A file of enums, inputs and choices, with no states. It compiles to plain functions and cannot run. |
| **Directive** | A line at the top of a file that starts with `@!`, such as `@!frontend:purescript`. |
| **Enum** | A named set of members, declared with `enum`. A pure file's inputs and tags come from enums. |
| **Input** | A value a pure file's choices test: a `bool`, or one enum. Declared with `input`. |
