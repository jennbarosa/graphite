// One process in a durable-host crash test. Each invocation is a fresh process, as a real restart
// would be.
//
//   durable_worker.ts <module> <state> <log> <lints>
//
// `<lints>` is a comma-separated list of lint outcomes: `pass`, `fail` or `throw`. Each task run
// appends its name and key to `<log>`, so a test can count real runs. With GRAPHITE_CRASH set to
// `<checkpoint>:<node>`, the process exits with CRASH_EXIT at that checkpoint and prints nothing.
import { appendFileSync } from 'node:fs';
import { type Checkpoint, type Outcome, runDurable } from '../../src/frontend/typescript/durable/engine';

const CRASH_EXIT = 7;
const LINT_TASK = 'lint';

const [modulePath = '', statePath = '', logPath = '', lints = ''] =
	process.argv.slice(2);
const script: string[] = lints === '' ? [] : lints.split(',');
const crashAt = process.env.GRAPHITE_CRASH ?? '';

/** The node a checkpoint key belongs to: keys are `<transitions>:<node>`. */
function nodeOf(key: string): string {
	return key.slice(key.indexOf(':') + 1);
}

function nextLintOutcome(): Outcome | 'throw' {
	const next = script.shift();
	if (next === 'pass' || next === 'fail' || next === 'throw') return next;
	throw new Error(`lint has no scripted outcome (got ${String(next)})`);
}

const result = await runDurable({
	modulePath,
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions: { pass: 'passed', fail: 'failed' },
	stateFile: statePath,
	taskTimeoutMs: 5_000,
	runner: async (task, context) => {
		appendFileSync(logPath, `${task} ${context.key}\n`);
		if (task !== LINT_TASK) return 'none';
		const outcome = nextLintOutcome();
		if (outcome === 'throw') throw new Error('lint crashed');
		return outcome;
	},
	onCheckpoint: (label: Checkpoint, key: string) => {
		if (crashAt === `${label}:${nodeOf(key)}`) process.exit(CRASH_EXIT);
	},
});

console.log(JSON.stringify(result));
