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
