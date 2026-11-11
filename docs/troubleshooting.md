# Troubleshooting

Match the message you see, then follow the fix. Messages are quoted exactly as Graphite prints them.

## Graph errors

**`unknown node; declare it with `node` before linking to it`**
An `edge` names a stage that is not declared, or a name is misspelled. Check the spelling against
the `node` line. Declare every `node` before the first `edge` or `choose`.

**`unknown node flag; expected `loop`, `end`, or a flag from the schema`**
The flag is not one Graphite knows. Either fix the spelling, or allow the flag in an allowlist with
`node_flags` and pass it with `--allow`. See [allowlists](dsl.md#allowlists).

**`expected the end of the line`** on a `node` line with an annotation
Flags come before annotations. Write `node a end @task:deploy`, not `node a @task:deploy end`.

**`node name is already declared in this graph`**
Two stages share a name in the same graph. Rename one.

**A node has no outgoing edge**
Every node needs an outgoing edge, unless it is marked `end`. An `end` node may not have outgoing
edges.

## Compile errors

**`the runtime file `graphite.ts` already exists in the output directory`**
`compile` never overwrites. Remove the output directory, or pass a new `--out`:

```sh
rm -rf graphite-out/door
graphite compile door.dsl
```

**`Cannot find module './graphite'`** when importing a generated graph
The generated `door.ts` imports `./graphite`, relative to itself. Keep the whole output directory
together, and import from the directory it is in.

## Running and debugging

**`no JavaScript runtime found: install bun, or npm and tsx, or set GRAPHITE_JS_RUNNER`**
Install [Bun](https://bun.sh), or install `tsx` through npm, or set `GRAPHITE_JS_RUNNER` to a
command that runs TypeScript. Run `graphite doctor` to see what Graphite found.

**`cannot start GRAPHITE_JS_RUNNER ...`**
The command in `GRAPHITE_JS_RUNNER` does not exist or cannot start. Check the spelling, and check
that it is on your `PATH`.

**`"status":"stalled"` and `durable run did not end`**
No edge can be taken and no task is running. In a durable run only `passed` and `failed` are set
for you. See [when a run stalls](durable-workflows.md#when-a-run-stalls).

**`state ... is locked by process N`**
Another run is using that state file. Wait for it to finish. If that process is gone and the
message persists, delete the `.lock` file next to the state file.

**`warning: --runner runs ... as code`**
This is expected. It is a reminder that the runner has your permissions. Continue only if you wrote
or reviewed it.

## Options

**`unknown option --something`**
The option is not recognized, or it is for a different command. Run `graphite help <command>`.

**`check does not take --out`** (and similar)
That option belongs to another command. The message names both.

## Tests

**`cannot run "tsc"`**
The type-check test needs TypeScript 5 or later. Install it, or set `GRAPHITE_TSC` to its path.
`graphite doctor` reports the path it found.
