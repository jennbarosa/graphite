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

`retry` is a domain flag, and the task names are annotation values. Both must be allowed, in
[`workflow.toml`](../examples/workflow/workflow.toml):

```toml
node_flags = ["retry"]

[annotations]
task = ["write", "lint", "fix", "deploy"]
```

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

- `--allow` is optional, but the task names are checked against it.
- `--state` defaults to the file's name with `.state.json`, beside the file.
- `--runner` is optional. Without it, every task settles as `none`.
- `--durable` is accepted and does nothing. `run` is always durable.

> **Security.** `--runner` runs its file as code with your user's permissions. Use only runners
> you wrote or have reviewed.

The run log goes to stderr. The final line is JSON, for example `{"status":"ended","transitions":2}`.
The exit status is 0 when the run ends.

## Failure details

A runner can say why a stage failed. It returns an object instead of a bare outcome:

```ts
return { outcome: "fail", detail: { message: "2 checks failed", code: "lint", data: { checks: ["max-len"] } } };
```

- `message` is required. It is cut to 1,000 characters.
- `code` is optional, a short label to match on. At most 64 characters.
- `data` is optional JSON for a later task. At most 4,096 bytes when serialized.
- A runner that returns a detail outside these limits fails that attempt, as if it had thrown.

The host keeps the most recent failed stage of the run, which is a `fail` or an `error` (a task
that threw on every attempt, whose detail is the thrown message). Later tasks see it as
`context.failure`, with the stage's `node`, `task`, `key`, `outcome` and `detail`. A later stage
that passes does not clear it. `context.failure` is `undefined` until a stage fails. The failure is
saved with the run's position, so a resumed run gives the same value.

The run log prints the detail after the outcome, for example
`[task] lint fail (ran, key 1:check) — lint: 2 checks failed`.

## What the host guarantees

- **Checkpoints.** The state is saved after every result, in `<state>` and the results in
  `<state>.results`. Running the same command again resumes where it stopped.
- **Outcomes.** `pass` and `fail` set the `passed` and `failed` conditions when the graph declares
  them. `none` sets neither. A node without a task is settled as `none`.
- **Retries.** Nodes flagged `retry` get three attempts when a task throws. A task that still
  fails stops the run with an error naming its key.
- **At least once.** A task that was running when the process died runs again on resume, with the
  same `key`. Tasks with external effects must dedupe on `key`.
- **Timeouts.** A task that exceeds `taskTimeoutMs` (default 60 s) counts as a failed attempt. The
  host cannot stop a runner that ignores its `AbortSignal`.

## When a run stalls

A run stops with `"status":"stalled"` when no edge can be taken and no task is running. The usual
cause is a gate on a condition that no task sets. A gate such as `when keyup` waits for the host;
in a durable run, only `passed` and `failed` are set for you. If your graph needs other
conditions, use the [TypeScript host API](typescript.md) rather than `run`.

The TypeScript host API (`runDurable`, `fileStore`, `runLog`) is documented in
[`host/durable.ts`](../host/durable.ts) and [`host/run-log.ts`](../host/run-log.ts).
