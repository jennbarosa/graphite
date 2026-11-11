# Glossary

The words Graphite uses, in the order you meet them.

| Term | Meaning |
| --- | --- |
| **Graph** | A named set of stages and the rules between them. A `.dsl` file holds one or more graphs. |
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
| **Annotation** | A `@key:value` pair on a node or edge, such as `@anim:walk`. Graphite passes it through unchanged. |
| **Flag** | A bare word on a node, such as `loop` or `end`. Your schema can allow more. |
| **Tag** | A string an arm returns, such as `side`. Opaque to Graphite. |
| **Allowlist** | A TOML file that lists the flags and annotation values a graph may use, checked when the file is compiled. |
| **Task** | With `run`, the work a node does, named by its `@task:NAME` annotation. |
| **Runner** | The TypeScript file that performs tasks and reports each `pass`, `fail` or `none`. |
| **Outcome** | The result a task reports. `pass` sets the `passed` condition and `fail` sets `failed`. |
| **Checkpoint** | The saved state a durable run writes after each result, so it can resume. |
| **Target** | An output format. The only target today is `typescript`. |
