// The durable host on the review workflow: retries, timeouts, locking, and recovery after a crash
// at each checkpoint, with each crash in its own process. Compiled and run by `tests/durable.rs`.
import { describe, expect, test } from 'bun:test';
import {
	existsSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	writeFileSync,
} from 'node:fs';
import { join, resolve } from 'node:path';
import { runLog } from '../../host/run-log';
import {
	type HostEvent,
	type Outcome,
	type Runner,
	type StateStore,
	MAX_DETAIL_CODE,
	MAX_DETAIL_DATA_BYTES,
	MAX_DETAIL_MESSAGE,
	type StageFailure,
	type StoredResult,
	fileStore,
	retryFailedTask,
	runDurable,
} from '../../host/durable';

const graphModule = process.env.GRAPHITE_WORKFLOW;
const stateDirVariable = process.env.GRAPHITE_STATE_DIR;
if (graphModule === undefined || stateDirVariable === undefined) {
	throw new Error('GRAPHITE_WORKFLOW and GRAPHITE_STATE_DIR must be set');
}
const stateBase: string = stateDirVariable;
const worker = resolve(import.meta.dir, 'durable_worker.ts');
/** The names this review workflow uses for its tasks, flags and outcome conditions. */
const workflowNames = {
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions: { pass: 'passed', fail: 'failed' },
};
/** The exit code the worker uses for a simulated crash. */
const CRASH_EXIT = 7;

interface Files {
	readonly dir: string;
	readonly state: string;
	readonly log: string;
}

/** A fresh directory with its own state file and task log. */
function scratch(): Files {
	const dir = mkdtempSync(join(stateBase, 'case-'));
	return { dir, state: join(dir, 'state.json'), log: join(dir, 'log.txt') };
}

/** The task-log lines that start with `task`, one per real run of that task. */
function runs(files: Files, task: string): string[] {
	if (!existsSync(files.log)) return [];
	return readFileSync(files.log, 'utf8')
		.split('\n')
		.filter((line) => line.startsWith(`${task} `));
}

/** Runs the worker in its own process. `crash` is `<checkpoint>:<node>`, or empty. */
function runWorker(files: Files, lints: string, crash = '') {
	const result = Bun.spawnSync(
		[
			process.execPath,
			worker,
			graphModule!,
			files.state,
			files.log,
			lints,
		],
		{ env: { ...process.env, GRAPHITE_CRASH: crash } }
	);
	return {
		code: result.exitCode,
		stdout: result.stdout.toString(),
		stderr: result.stderr.toString(),
	};
}

/** A runner that records its calls and plays scripted outcomes for `lint`. */
function recorder(script: Array<Outcome | 'throw'> = []) {
	const calls: Array<{ task: string; attempt: number; key: string }> = [];
	const queue = [...script];
	const runner: Runner = async (task, context) => {
		calls.push({ task, attempt: context.attempt, key: context.key });
		if (task !== 'lint') return 'none';
		const next = queue.shift() ?? 'pass';
		if (next === 'throw') throw new Error('lint crashed');
		return next;
	};
	return { runner, calls };
}

/** One line of the results file, as written. */
interface RecordLine {
	readonly key: string;
	readonly sequence: number;
	readonly outcome: string | null;
	readonly detail?: { readonly message: string; readonly code?: string };
}

function recordsOf(files: Files): RecordLine[] {
	return readFileSync(`${files.state}.results`, 'utf8')
		.trim()
		.split('\n')
		.map((line) => JSON.parse(line) as RecordLine);
}

/** Runs the review workflow until its `fix` stage errors, so the run stops after a lint failure. */
async function stopAtFix(files: Files, input?: unknown) {
	let lints = 0;
	const result = await runDurable({
		modulePath: graphModule,
		...workflowNames,
		stateFile: files.state,
		maxAttempts: 1,
		input,
		runner: async (task) => {
			if (task === 'fix') throw new Error('fixer is down');
			if (task !== 'lint') return 'none';
			return lints++ === 0
				? { outcome: 'fail', detail: { message: 'lint: 3 errors' } }
				: 'pass';
		},
	});
	if (result.status !== 'error') throw new Error('expected the run to stop at fix');
	return result;
}

describe('durable host', () => {
	test('a failing lint goes through fix and back, then ships, and every task runs once', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['fail', 'pass']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result).toEqual({
			status: 'ended',
			transitions: 4,
			node: 'ship',
			outcome: 'none',
		});
		expect(calls.map((call) => call.task)).toEqual([
			'write',
			'lint',
			'fix',
			'lint',
			'deploy',
		]);
		expect(
			readdirSync(files.dir).filter((name) => name.endsWith('.tmp'))
		).toEqual([]);
	});

	test('reaching an end node runs its task before the run reports ended', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['pass']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result).toEqual({
			status: 'ended',
			transitions: 2,
			node: 'ship',
			outcome: 'none',
		});
		expect(calls.map((call) => call.task)).toEqual([
			'write',
			'lint',
			'deploy',
		]);
	});

	test('a retry node retries a throwing task, and the attempt number is passed to the runner', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['throw', 'pass']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result.status).toBe('ended');
		expect(
			calls
				.filter((call) => call.task === 'lint')
				.map((call) => call.attempt)
		).toEqual([1, 2]);
	});

	test('a retry node that keeps throwing stops with an error, and the stored error is not retried', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['throw', 'throw', 'throw']);
		const options = {
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxAttempts: 3,
		};
		const lintCalls = () =>
			calls.filter((call) => call.task === 'lint').length;

		await expect(runDurable(options)).resolves.toMatchObject({
			status: 'error',
			error: 'task lint failed: lint crashed',
		});
		expect(lintCalls()).toBe(3);

		await expect(runDurable(options)).resolves.toMatchObject({
			status: 'error',
			error: 'task lint failed in an earlier run',
		});
		expect(lintCalls()).toBe(3);
	});

	test('a stored error is cleared by retryFailedTask, and the next run runs that task again', async () => {
		const files = scratch();
		const { runner, calls } = recorder([
			'throw',
			'throw',
			'throw',
			'pass',
		]);
		const options = {
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxAttempts: 3,
		};
		const failed = await runDurable(options);
		expect(failed).toMatchObject({ status: 'error' });
		const key = failed.status === 'error' ? failed.key : '';

		await retryFailedTask(fileStore(files.state), key);
		const resumed = await runDurable(options);
		expect(resumed).toEqual({
			status: 'ended',
			transitions: 2,
			node: 'ship',
			outcome: 'none',
		});
		expect(calls.filter((call) => call.task === 'lint')).toHaveLength(
			4
		);
	});

	test('retryFailedTask refuses a key with no stored error', async () => {
		const files = scratch();
		const { runner } = recorder();
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		await expect(
			retryFailedTask(fileStore(files.state), '0:check')
		).rejects.toThrow('has no stored error');
	});

	test('a run uses the store it is given, not the state file', async () => {
		const files = scratch();
		let snapshot: unknown;
		const records: {
			key: string;
			sequence: number;
			result: StoredResult | null;
		}[] = [];
		const memory: StateStore = {
			loadSnapshot: () => snapshot,
			loadRecords: (since) => {
				const latest = new Map<string, StoredResult | null>();
				for (const record of records) {
					if (record.sequence >= since)
						latest.set(record.key, record.result);
				}
				return new Map(
					[...latest].flatMap(([key, result]) =>
						result === null ? [] : [[key, result] as const]
					)
				);
			},
			saveSnapshot: (value) => {
				snapshot = JSON.parse(JSON.stringify(value));
			},
			recordResult: (key, sequence, result) => {
				records.push({ key, sequence, result: result ?? null });
			},
			withLock: async (body) => body(),
		};
		const { runner } = recorder();
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			store: memory,
			runner,
		});
		expect(result.status).toBe('ended');
		expect(snapshot).toBeDefined();
		expect(records).toHaveLength(3);
		expect(existsSync(files.state)).toBe(false);
	});

	test('the file store loads only the records from a sequence onward', async () => {
		const files = scratch();
		const { runner } = recorder(['fail', 'pass']);
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		const store = fileStore(files.state);
		expect([...store.loadRecords(0).keys()]).toEqual([
			'0:draft',
			'1:check',
			'2:fix',
			'3:check',
			'4:ship',
		]);
		expect([...store.loadRecords(3).keys()]).toEqual([
			'3:check',
			'4:ship',
		]);
	});

	test('the snapshot holds the position only; each result is appended on its own line', async () => {
		const files = scratch();
		const { runner } = recorder(['fail', 'pass']);
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		const snapshot = JSON.parse(readFileSync(files.state, 'utf8'));
		expect(JSON.stringify(snapshot)).not.toContain('"results"');
		const lines = readFileSync(`${files.state}.results`, 'utf8')
			.trim()
			.split('\n');
		// write, lint (fail), fix, lint (pass), deploy: one line per recorded task
		expect(lines).toHaveLength(5);
		expect(lines.map((line) => JSON.parse(line).key)).toEqual([
			'0:draft',
			'1:check',
			'2:fix',
			'3:check',
			'4:ship',
		]);
	});

	test('a last results line cut short by a crash is dropped, and the run still resumes', async () => {
		const files = scratch();
		const { runner } = recorder(['pass']);
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		writeFileSync(
			`${files.state}.results`,
			readFileSync(`${files.state}.results`, 'utf8') +
				'{"key":"9:tor'
		);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result.status).toBe('ended');
		expect(
			readFileSync(`${files.state}.results`, 'utf8')
		).not.toContain('tor');
	});

	test('a graph without the outcome conditions is refused before anything runs', async () => {
		const files = scratch();
		const { runner, calls } = recorder();
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				outcomeConditions: {
					pass: 'no-such-condition',
					fail: 'failed',
				},
				stateFile: files.state,
				runner,
			})
		).rejects.toThrow('has no condition "no-such-condition"');
		expect(calls).toEqual([]);
	});

	test('reaching the transition limit says which limit was reached', async () => {
		const files = scratch();
		const { runner } = recorder(['fail', 'fail', 'fail']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxTransitions: 2,
		});
		expect(result).toMatchObject({
			status: 'error',
			error: 'run reached maxTransitions (2)',
		});
	});

	test('a node without retry gets one attempt', async () => {
		const files = scratch();
		const { runner } = recorder();
		let deployAttempts = 0;
		const deploying: Runner = async (task, context) => {
			if (task === 'deploy') {
				deployAttempts++;
				throw new Error('deploy refused');
			}
			return runner(task, context);
		};
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: deploying,
		});
		expect(result).toMatchObject({
			status: 'error',
			error: 'task deploy failed: deploy refused',
		});
		expect(deployAttempts).toBe(1);
	});

	test('a task that outlives its timeout is an error, not a hang', async () => {
		const files = scratch();
		const hangs: Runner = () => new Promise<never>(() => {});
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: hangs,
			taskTimeoutMs: 20,
		});
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
	});

	test('a runner that returns something other than an outcome is an error', async () => {
		const files = scratch();
		const odd = (async () => 'maybe') as unknown as Runner;
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: odd,
		});
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: runner returned "maybe"',
		});
	});

	test('a maxAttempts below one is refused before anything runs', async () => {
		const files = scratch();
		const { runner, calls } = recorder();
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner,
				maxAttempts: 0,
			})
		).rejects.toThrow('maxAttempts must be a positive integer');
		expect(calls).toEqual([]);
	});

	test('a state file held by a running process is refused', async () => {
		const files = scratch();
		writeFileSync(`${files.state}.lock`, String(process.pid));
		const { runner } = recorder();
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner,
			})
		).rejects.toThrow(`locked by process ${process.pid}`);
	});

	test('a lock left by a dead process is taken over', async () => {
		const files = scratch();
		const gone = Bun.spawn([process.execPath, '-e', '0']);
		await gone.exited;
		writeFileSync(`${files.state}.lock`, String(gone.pid));
		const { runner } = recorder();
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result.status).toBe('ended');
		expect(existsSync(`${files.state}.lock`)).toBe(false);
	});

	test('a crash after a task ran but before its result was saved reruns the task with the same key', () => {
		const files = scratch();
		expect(runWorker(files, 'pass', 'task-done:check').code).toBe(
			CRASH_EXIT
		);
		expect(runs(files, 'lint')).toHaveLength(1);

		// The resumed process runs the lint again, because its result was never saved.
		const finished = JSON.parse(runWorker(files, 'pass').stdout);
		expect(finished).toMatchObject({
			status: 'ended',
			transitions: 2,
			node: 'ship',
		});
		const lintRuns = runs(files, 'lint');
		expect(lintRuns).toHaveLength(2);
		expect(new Set(lintRuns).size).toBe(1);
		expect(runs(files, 'deploy')).toHaveLength(1);
	});

	test('a crash after the result was saved resumes without rerunning the task', () => {
		const files = scratch();
		expect(runWorker(files, 'pass', 'result-saved:check').code).toBe(
			CRASH_EXIT
		);
		const finished = JSON.parse(runWorker(files, '').stdout);
		expect(finished.status).toBe('ended');
		expect(runs(files, 'lint')).toHaveLength(1);
		expect(runs(files, 'deploy')).toHaveLength(1);
	});

	test('a crash after the step was taken but before it was saved replays the step without rerunning the task', () => {
		const files = scratch();
		expect(runWorker(files, 'pass', 'step-saved:check').code).toBe(
			CRASH_EXIT
		);
		const finished = JSON.parse(runWorker(files, '').stdout);
		expect(finished.status).toBe('ended');
		expect(runs(files, 'lint')).toHaveLength(1);
	});
	test('a task whose effect already happened is not run again when verify finds it', async () => {
		const files = scratch();
		const effects = new Map<string, Outcome>();
		let lintRuns = 0;
		const runner: Runner = async (task, context) => {
			if (task !== 'lint') return 'none';
			lintRuns++;
			effects.set(context.key, 'pass');
			return 'pass';
		};
		const verify = async (task: string, key: string) =>
			task === 'lint' ? effects.get(key) : undefined;
		const crashAfterEffect = (label: string, key: string) => {
			if (label === 'task-done' && key.endsWith(':check')) {
				throw new Error('simulated crash');
			}
		};

		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner,
				verify,
				onCheckpoint: crashAfterEffect,
			})
		).rejects.toThrow('simulated crash');
		expect(lintRuns).toBe(1);

		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			verify,
		});
		expect(result.status).toBe('ended');
		expect(lintRuns).toBe(1);
	});
	test('the task annotation, retry flag and outcome conditions are options, not fixed names', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['throw']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxAttempts: 3,
			taskAnnotation: 'no-such-annotation',
			retryFlag: 'no-such-flag',
		});
		// No task runs, so neither outcome condition is set and the run waits at check.
		expect(result).toEqual({
			status: 'stalled',
			transitions: 1,
			node: 'check',
		});
		expect(calls).toEqual([]);
	});

	test('a retry flag that the graph does not use gives one attempt', async () => {
		const files = scratch();
		const { runner, calls } = recorder(['throw', 'throw', 'throw']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxAttempts: 3,
			retryFlag: 'no-such-flag',
		});
		expect(result).toMatchObject({
			status: 'error',
			error: 'task lint failed: lint crashed',
		});
		expect(calls.filter((call) => call.task === 'lint')).toHaveLength(
			1
		);
	});
	test('onEvent reports each stage, each task outcome with its source, failed attempts, and the stop', async () => {
		const files = scratch();
		const events: HostEvent[] = [];
		const { runner } = recorder(['throw', 'pass']);
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			maxAttempts: 2,
			onEvent: (event) => events.push(event),
		});
		expect(result.status).toBe('ended');
		expect(
			events
				.filter((e) => e.type === 'stage-entered')
				.map((e) => e.node)
		).toEqual(['draft', 'check', 'ship']);
		expect(events.filter((e) => e.type === 'attempt-failed')).toEqual([
			expect.objectContaining({
				task: 'lint',
				attempt: 1,
				error: 'lint crashed',
			}),
		]);
		expect(
			events
				.filter((e) => e.type === 'task-settled')
				.map((e) => [e.task, e.source])
		).toEqual([
			['write', 'ran'],
			['lint', 'ran'],
			['deploy', 'ran'],
		]);
		expect(events.at(-1)).toEqual({ type: 'run-stopped', result });
	});

	test('a task settled by verify reports source verified, and no attempt runs', async () => {
		const files = scratch();
		const events: HostEvent[] = [];
		const { runner, calls } = recorder();
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			verify: async (task) =>
				task === 'lint' ? 'pass' : undefined,
			onEvent: (event) => events.push(event),
		});
		expect(calls.filter((call) => call.task === 'lint')).toEqual([]);
		expect(events).toContainEqual(
			expect.objectContaining({
				type: 'task-settled',
				task: 'lint',
				source: 'verified',
				outcome: 'pass',
			})
		);
	});
	test('runLog writes the run start, each stage and task, and a summary that counts repeats', async () => {
		const files = scratch();
		const lines: string[] = [];
		const { runner } = recorder(['fail', 'pass']);
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			onEvent: runLog((line) => lines.push(line), {
				timestamps: false,
			}),
		});
		expect(lines[0]).toBe('[run] started fresh, 0 stored result(s)');
		expect(lines).toContain('[task] lint fail (ran, key 1:check)');
		expect(lines.at(-2)).toBe(
			'[run] stages: draft, check x2, fix, ship'
		);
		expect(lines.at(-1)).toBe(
			'[run] tasks: write, lint x2, fix, deploy'
		);
		expect(lines).toContain('[run] stopped ended');
	});

	test('runLog says a run resumed after a crash, and shows where the crash was', async () => {
		const files = scratch();
		const crashed: string[] = [];
		const { runner } = recorder(['pass']);
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner,
				onEvent: runLog((line) => crashed.push(line), {
					timestamps: false,
				}),
				onCheckpoint: (label, key) => {
					if (
						label === 'task-done' &&
						key.endsWith(':check')
					) {
						throw new Error('simulated crash');
					}
				},
			})
		).rejects.toThrow('simulated crash');
		// The crash came before lint's result was settled, so the log ends at the stage it was in.
		expect(crashed.at(-1)).toBe('[stage] check (key 1:check)');

		const resumed: string[] = [];
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			onEvent: runLog((line) => resumed.push(line), {
				timestamps: false,
			}),
		});
		expect(resumed[0]).toBe(
			'[run] started resumed, 0 stored result(s)'
		);
		expect(resumed).toContain('[stage] check (key 1:check)');
	});
	test('runLog puts a timestamp before each line, from the clock it is given', async () => {
		const files = scratch();
		const lines: string[] = [];
		const { runner } = recorder();
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
			onEvent: runLog((line) => lines.push(line), {
				clock: () => '12:00:00',
			}),
		});
		expect(lines.length).toBeGreaterThan(0);
		for (const line of lines)
			expect(line.startsWith('12:00:00 [')).toBe(true);
	});

	test('a failed stage passes its detail to later tasks as context.failure, and a later pass keeps it', async () => {
		const files = scratch();
		const seen: { task: string; failure: StageFailure | undefined }[] = [];
		let lints = 0;
		const runner: Runner = async (task, context) => {
			seen.push({ task, failure: context.failure });
			if (task !== 'lint') return 'none';
			return lints++ === 0
				? {
						outcome: 'fail',
						detail: {
							message: 'two checks failed',
							code: 'lint',
							data: { checks: ['no-unused', 'max-len'] },
						},
					}
				: 'pass';
		};
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner,
		});
		expect(result.status).toBe('ended');
		const failure: StageFailure = {
			node: 'check',
			task: 'lint',
			key: '1:check',
			outcome: 'fail',
			detail: {
				message: 'two checks failed',
				code: 'lint',
				data: { checks: ['no-unused', 'max-len'] },
			},
		};
		expect(seen.find((call) => call.task === 'write')?.failure).toBeUndefined();
		expect(seen.find((call) => call.task === 'fix')?.failure).toEqual(failure);
		// The second lint passed, and deploy still sees the earlier failure.
		expect(seen.filter((call) => call.task === 'deploy')[0]?.failure).toEqual(failure);
	});

	test('a resumed run gives the next task the failure from before the stop', async () => {
		const files = scratch();
		const stopped = await stopAtFix(files);
		await retryFailedTask(fileStore(files.state), stopped.key);
		const seen: (StageFailure | undefined)[] = [];
		const resumed = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: async (task, context) => {
				if (task === 'fix') seen.push(context.failure);
				return task === 'lint' ? 'pass' : 'none';
			},
		});
		expect(resumed.status).toBe('ended');
		expect(seen).toEqual([
			{
				node: 'check',
				task: 'lint',
				key: '1:check',
				outcome: 'fail',
				detail: { message: 'lint: 3 errors' },
			},
		]);
	});

	test('a state file saved before failures were kept still resumes, with no failure', async () => {
		const files = scratch();
		const stopped = await stopAtFix(files);
		await retryFailedTask(fileStore(files.state), stopped.key);
		// Rewrite the snapshot in the older shape: the run's own snapshot, with no wrapper.
		const saved = JSON.parse(readFileSync(files.state, 'utf8'));
		writeFileSync(
			files.state,
			JSON.stringify({ snapshot: saved.snapshot.run })
		);
		const seen: (StageFailure | undefined)[] = [];
		const resumed = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: async (task, context) => {
				if (task === 'fix') seen.push(context.failure);
				return task === 'lint' ? 'pass' : 'none';
			},
		});
		expect(resumed.status).toBe('ended');
		expect(seen).toEqual([undefined]);
	});

	test('a message over the limit is cut with an ellipsis, and data over the limit stops the attempt', async () => {
		const files = scratch();
		const seen: (StageFailure | undefined)[] = [];
		let lintsRun = 0;
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			maxAttempts: 1,
			runner: async (task, context) => {
				if (task === 'fix') seen.push(context.failure);
				if (task !== 'lint') return 'none';
				return lintsRun++ === 0
					? {
							outcome: 'fail',
							detail: { message: 'x'.repeat(MAX_DETAIL_MESSAGE + 500) },
						}
					: 'pass';
			},
		});
		expect(result.status).toBe('ended');
		const message = seen[0]?.detail?.message ?? '';
		expect(message.length).toBe(MAX_DETAIL_MESSAGE);
		expect(message.endsWith('…')).toBe(true);

		const oversized = scratch();
		const refused = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: oversized.state,
			maxAttempts: 1,
			runner: async (task) =>
				task === 'lint'
					? {
							outcome: 'fail',
							detail: {
								message: 'too much data',
								data: 'y'.repeat(MAX_DETAIL_DATA_BYTES + 1),
							},
						}
					: 'none',
		});
		expect(refused.status).toBe('error');
		if (refused.status !== 'error') throw new Error('unreachable');
		expect(refused.error).toContain(`at most ${MAX_DETAIL_DATA_BYTES} bytes`);
	});

	test('runLog prints the detail a runner gave with its outcome', async () => {
		const files = scratch();
		const lines: string[] = [];
		let lints = 0;
		await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: async (task) => {
				if (task !== 'lint') return 'none';
				return lints++ === 0
					? {
							outcome: 'fail',
							detail: { message: 'two checks failed', code: 'lint' },
						}
					: 'pass';
			},
			onEvent: runLog((line) => lines.push(line), { timestamps: false }),
		});
		expect(lines).toContain(
			'[task] lint fail (ran, key 1:check) — lint: two checks failed'
		);
	});

	test('a task that throws stores its message as the error detail, capped like any detail', async () => {
		const files = scratch();
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			maxAttempts: 1,
			runner: async (task) => {
				if (task === 'lint') throw new Error('x'.repeat(MAX_DETAIL_MESSAGE + 10));
				return 'none';
			},
		});
		expect(result.status).toBe('error');
		const record = recordsOf(files).find((line) => line.key === '1:check');
		expect(record?.outcome).toBe('error');
		expect(record?.detail?.message).toBe(`${'x'.repeat(MAX_DETAIL_MESSAGE - 1)}…`);
		expect(record?.detail?.code).toBeUndefined();
	});

	test('a detail code over the limit, or not a string, is refused and the attempt fails', async () => {
		for (const code of ['c'.repeat(MAX_DETAIL_CODE + 1), 7]) {
			const files = scratch();
			const result = await runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				maxAttempts: 1,
				runner: async (task) =>
					task === 'lint'
						? JSON.parse(
								JSON.stringify({ outcome: 'fail', detail: { message: 'x', code } })
							)
						: 'none',
			});
			expect(result.status).toBe('error');
			if (result.status !== 'error') throw new Error('unreachable');
			expect(result.error).toContain(
				`code must be a string of at most ${MAX_DETAIL_CODE} characters`
			);
		}
	});

	test('a runner object whose outcome is not an outcome is an error', async () => {
		const files = scratch();
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			maxAttempts: 1,
			runner: async (task) =>
				task === 'lint' ? JSON.parse('{"outcome":"maybe"}') : 'none',
		});
		expect(result.status).toBe('error');
		if (result.status !== 'error') throw new Error('unreachable');
		expect(result.error).toContain('runner returned an object whose outcome is "maybe"');
	});

	test('a runner detail without a string message is an error', async () => {
		const files = scratch();
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			maxAttempts: 1,
			runner: async (task) =>
				task === 'lint'
					? JSON.parse('{"outcome":"fail","detail":{"code":"lint"}}')
					: 'none',
		});
		expect(result.status).toBe('error');
		if (result.status !== 'error') throw new Error('unreachable');
		expect(result.error).toContain('runner detail needs a string message');
	});

	test('a saved last failure that is missing its fields stops the resume, rather than being guessed at', async () => {
		const files = scratch();
		await stopAtFix(files);
		const saved = JSON.parse(readFileSync(files.state, 'utf8'));
		saved.snapshot.lastFailure = { node: 'check' };
		writeFileSync(files.state, JSON.stringify(saved));
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner: async () => 'pass',
			})
		).rejects.toThrow('state file is malformed: lastFailure');
	});

	test('a results line with a non-object detail stops the resume as a damaged file', async () => {
		const files = scratch();
		await stopAtFix(files);
		const sequence = recordsOf(files).at(-1)?.sequence ?? 0;
		writeFileSync(
			`${files.state}.results`,
			`${readFileSync(`${files.state}.results`, 'utf8')}${JSON.stringify({
				key: `${sequence}:fix`,
				sequence,
				outcome: 'pass',
				detail: 5,
			})}\n`
		);
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				runner: async () => 'pass',
			})
		).rejects.toThrow(/results file .* is malformed/);
	});

	test('a resumed run gets the input it started with, and a different input is refused', async () => {
		const files = scratch();
		const stopped = await stopAtFix(files, { target: 'prospect-1' });
		await retryFailedTask(fileStore(files.state), stopped.key);
		const seen: unknown[] = [];
		await expect(
			runDurable({
				modulePath: graphModule,
				...workflowNames,
				stateFile: files.state,
				input: { target: 'prospect-2' },
				runner: async () => 'pass',
			})
		).rejects.toThrow('the input does not match the input this run started with');
		const resumed = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			input: { target: 'prospect-1' },
			runner: async (task, context) => {
				seen.push(context.input);
				return task === 'lint' ? 'pass' : 'none';
			},
		});
		expect(resumed.status).toBe('ended');
		expect(seen.every((input) => JSON.stringify(input) === '{"target":"prospect-1"}')).toBe(true);
	});

	test('a task saves an output that a later task reads by node name, and the end node reports its output', async () => {
		const files = scratch();
		const seen: unknown[] = [];
		const result = await runDurable({
			modulePath: graphModule,
			...workflowNames,
			stateFile: files.state,
			runner: async (task, context) => {
				if (task === 'write') return { outcome: 'none', output: { draft: 'hello' } };
				if (task === 'lint') {
					seen.push(await context.outputs.read('draft'));
					seen.push(await context.outputs.read('nothing'));
					return 'pass';
				}
				return { outcome: 'none', output: { shipped: true } };
			},
		});
		expect(seen).toEqual([{ draft: 'hello' }, undefined]);
		expect(result).toEqual({
			status: 'ended',
			transitions: 2,
			node: 'ship',
			outcome: 'none',
			output: { shipped: true },
		});
		expect(readFileSync(join(`${files.state}.outputs`, 'draft.json'), 'utf8')).toBe(
			'{"draft":"hello"}'
		);
	});
});
