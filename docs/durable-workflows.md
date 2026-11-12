# Durable workflows

`graphite run` executes the first graph in a file as a workflow. A node with a `@task:NAME`
annotation runs that task through a **runner** that you supply. The runner reports an
**outcome**, and the graph routes on it. Every result is saved, so a run that stops can resume
without repeating finished work.

## A workflow

[`examples/workflow/workflow.dsl`](../examples/workflow/workflow.dsl):

```text
graph review
  node draft   @task:write
  node check   retry @task:lint
  node fix     @task:fix
  node ship    end @task:deploy

  edge draft -> check
  edge check -> ship when passed @report:ok
  edge check -> fix  when failed
  edge fix   -> check
```

`retry` is a domain flag, and the task names are annotation values. Both must be allowed. The
allowlist (`--allow`) lists the flag and the task names, in [`workflow.toml`](../examples/workflow/workflow.toml).
It is a list of names the graph may use. It does not define any task, and it holds no commands:

```toml
node_flags = ["retry"]

[annotations]
task = ["write", "lint", "fix", "deploy"]
```

With a `tasks.toml` (see [command tasks](#command-tasks)), the task names come from its `[tasks]` keys, so
the allowlist only needs `node_flags`.

### Where each thing goes

| File | Passed as | Holds | Never holds |
|---|---|---|---|
| graph (`.dsl`) | the first argument | nodes, edges, `@task:<name>` | commands |
| allowlist (`workflow.toml` above) | `--allow` | `node_flags`, and allowed task names under `[annotations]` | commands, or a `[tasks]` table |
| task manifest (`tasks.toml`) | `--tasks` | `[tasks]`: each task name mapped to its command | flags, graph structure |

A task's command goes only in the `[tasks]` table of a `tasks.toml`. An allowlist with a `[tasks]`
section is refused, because its only sections are `[annotations]` and `node_flags`.

## The runner

The runner's default export is called with each task name and a context, and returns `"pass"`,
`"fail"`, or `"none"`. See [`runner.ts`](../examples/workflow/runner.ts):

```ts
export default async function (task: string, context: { key: string; attempt: number }) {
  console.error(`running ${task} (${context.key}, attempt ${context.attempt})`);
  return "pass" as const;
}
```

## Running it

```sh
graphite run examples/workflow/workflow.dsl \
  --allow examples/workflow/workflow.toml \
  --state target/review.state.json \
  --runner examples/workflow/runner.ts
```

- `--allow` is optional, but the task names are checked against it, or against `--tasks` when given.
- `--state` defaults to the file's name with `.state.json`, beside the file.
- `--runner` is optional. Without it, every task settles as `none`.
- `--durable` is accepted and does nothing. `run` is always durable.

> **Security.** `--runner` runs its file as code with your user's permissions. Use only runners
> you wrote or have reviewed.

The run log goes to stderr. The final line is JSON, for example
`{"status":"ended","transitions":2,"node":"ship","outcome":"pass"}`: the end node's name and its
stage's outcome.
The exit status is 0 when the run ends.

## Command tasks

Instead of a runner module, a manifest can name the command that runs each task. Commands run in
the directory that holds the manifest, so relative paths in it are relative to the manifest:

```toml
[tasks]
write = ["npm", "run", "write"]
lint = ["python3", "tools/lint.py"]
```

```sh
graphite run review.dsl --tasks tasks.toml
```

A task's command gets one JSON object on stdin, `{ "task", "key", "attempt", "input", "failure" }`.
`input` is the run's input, or `null`. `failure` is the run's last failed stage, as
`context.failure` would show it, or `null`. The command reports one JSON object on stdout, with the
same shape a runner returns:

```json
{"outcome": "fail", "detail": {"message": "2 checks failed", "code": "lint"}}
```

- Empty output with exit status 0 is a `pass`.
- A nonzero exit status is a thrown attempt, so a node flagged `retry` tries again.
- Output that is not JSON is a thrown attempt.
- Stderr goes to the terminal, next to the run log.
- A command that outlives `taskTimeoutMs` is stopped.

Every task in the graph needs a command. A graph whose task has no command is refused before
anything is recorded, and the error names the task. The commands run with your permissions, like
`--runner`: use only commands you wrote or have reviewed.

## Input

`--input` names a JSON file whose value every task receives as `context.input`, and as `input` on
stdin for a command task. Use it for the target a graph runs against, so the same graph can run
against different targets without changing it.

A resumed run must be given the same input. The engine keeps a fingerprint of the input in the state
file, and refuses a resume with a different one, so no task sees a mix of two inputs.

## Stage outputs

A task can keep a JSON output for the tasks after it. The runner returns it with its outcome:

```ts
return { outcome: "none", output: { draft: "Hi, ..." } };
```

A later task reads it by the node's name, with `context.outputs.read("draft")`, which returns `undefined`
when the node saved none. A command task finds the directory in its request as `outputs`, and reads
`<dir>/<node>.json`. The output is saved before the result is recorded, so a crash between the two
reruns the task and overwrites it. At most 1 MiB of JSON per output.

When the run reaches an end node, the result carries that node's name and its output:
`{"status":"ended","transitions":2,"node":"ship","outcome":"pass","output":{"shipped":true}}`.

## Error routing

A graph that declares an `errored` condition routes a task that throws on every attempt, rather than
stopping the run:

```text
edge check -> fix when errored
```

The stage's result is `error`, with the thrown message as its detail, and it becomes the run's failure.
Without an `errored` condition, a thrown task stops the run, as before.

## Fan-out

A list graph can run another graph once per item it produces:

```sh
graphite run discover.dsl --each prospect.dsl --key domain --tasks tasks.toml \
  --seen seen.json --until-passed 10 --state today.state.json
```

The list graph must end with an array as its output, one object per item. Each item runs the
`--each` graph once, with the item as its input and its own state file under `<state>.items/`. Two
things keep the engine from doing the same work twice:

- **Dedupe.** `--key` names the item field that identifies it. The key of each item started is written
  to the seen file, which defaults to `<state>.seen.json`. The file holds one key per line, appended and
fsynced as each item starts, so an old JSON-array seen file must be deleted before it is reused. A key
already there is skipped. Keys must be
  letters, digits, dots, dashes or underscores, since each one is also a file name.
- **Counter.** `--until-passed N` stops starting items once N have ended on a passing stage. An item
  ends on a passing stage when its end node's own task returns `pass`.

An item that stops or errors does not count as passed, and its key stays in the seen file, so it is
not retried on the next run. Pass the same `--seen` file on each run to keep the list across days.

## Failure details

A runner can say why a stage failed. It returns an object instead of a bare outcome:

```ts
return { outcome: "fail", detail: { message: "2 checks failed", code: "lint", data: { checks: ["max-len"] } } };
```

- `message` is required. It is cut to 1,000 characters.
- `code` is optional, a short label to match on. At most 64 characters.
- `data` is optional JSON for a later task. At most 4,096 bytes when serialized.
- A runner that returns a detail outside these limits fails that attempt, as if it had thrown.

The engine keeps the most recent failed stage of the run, which is a `fail` or an `error` (a task
that threw on every attempt, whose detail is the thrown message). Later tasks see it as
`context.failure`, with the stage's `node`, `task`, `key`, `outcome` and `detail`. A later stage
that passes does not clear it. `context.failure` is `undefined` until a stage fails. The failure is
saved with the run's position, so a resumed run gives the same value.

The run log prints the detail after the outcome, for example
`[task] lint fail (ran, key 1:check) — lint: 2 checks failed`.

## What the engine guarantees

- **Checkpoints.** The state is saved after every result, in `<state>` and the results in
  `<state>.results`. Running the same command again resumes where it stopped.
- **Outcomes.** `pass` and `fail` set the `passed` and `failed` conditions when the graph declares
  them. `none` sets neither. A node without a task is settled as `none`.
- **Retries.** Nodes flagged `retry` get three attempts when a task throws. A task that still
  fails stops the run with an error naming its key.
- **At least once.** A task that was running when the process died runs again on resume, with the
  same `key`. Tasks with external effects must dedupe on `key`.
- **Timeouts.** A task that exceeds `taskTimeoutMs` (default 60 s) counts as a failed attempt. The
  engine cannot stop a runner that ignores its `AbortSignal`.

## When a run stalls

A run stops with `"status":"stalled"` when no edge can be taken and no task is running. The usual
cause is a gate on a condition that no task sets. A gate such as `when keyup` waits for the engine;
in a durable run, only `passed` and `failed` are set for you. If your graph needs other
conditions, use the [TypeScript API](typescript.md) rather than `run`.

The durable engine API (`runDurable`, `fileStore`, `runLog`) is documented in
[`engine.ts`](../src/frontend/typescript/durable/engine.ts) and [`run-log.ts`](../src/frontend/typescript/durable/run-log.ts).
