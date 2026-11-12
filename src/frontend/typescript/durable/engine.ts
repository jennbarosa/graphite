// Durable engine for a compiled graph. It runs each stage's task through a runner, saves state
// after every checkpoint, and resumes after a crash with the same path through the graph.
//
// Guarantees, for trusted DSL and internal use:
// - The graph is deterministic: given the same task outcomes, a run always takes the same path.
// - A task whose result was recorded is never run again. A task in flight when the process died is
//   run again on resume, with the same `key` as before. So a task may run more than once, and
//   any side effect it has must dedupe on `key` in the external system.
// - A task that throws is retried only on nodes flagged with the retry flag. A task that still
//   fails is recorded as `error` and stops the run; the recorded error is not retried.
// - A task that outlives `taskTimeoutMs` counts as a failed attempt. The runner gets an
//   AbortSignal, but a runner that ignores it keeps running in the background.
// - State lives in a `StateStore`. The run's position is a small snapshot. Each task result is
//   appended once. A run holds in memory only the results of the stage it is on, so memory does
//   not grow with the length of the run. Any other store must provide an atomic snapshot save, an
//   append that is durable when it returns, and an exclusive lock.
// - A recorded error stops the run until `retryFailedTask` clears it.

import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import type {
	Detail,
	GraphLike,
	Outcome,
	RunnerReport,
	StateStore,
	StoredOutcome,
	StoredResult,
} from './contract';
import { MAX_DETAIL_CODE, MAX_DETAIL_DATA_BYTES, MAX_DETAIL_MESSAGE } from './contract';
import { capMessage, isDetail, isOutcome, isRecord, normalizeReport } from './report';
import { taskOf } from './graph-checks';
import { fileStore } from './store';

export { MAX_DETAIL_CODE, MAX_DETAIL_DATA_BYTES, MAX_DETAIL_MESSAGE } from './contract';
export type { Detail, Outcome, RunnerReport, StateStore, StoredOutcome, StoredResult } from './contract';
export { fileStore } from './store';

const DEFAULT_MAX_TRANSITIONS = 10_000_000;
const DEFAULT_MAX_ATTEMPTS = 3;
const DEFAULT_TASK_TIMEOUT_MS = 60_000;

export interface TaskContext {
	/** Stable for a stage visit, including across a crash. External systems dedupe on it. */
	readonly key: string;
	readonly attempt: number;
	readonly signal: AbortSignal;
	/**
	 * The most recent failed stage of this run, including one from before a crash. Undefined when
	 * no stage has failed.
	 */
	readonly failure: StageFailure | undefined;
	/** The run's input, as given to `runDurable`. Undefined when the run has none. */
	readonly input: unknown;
	/** Outputs that earlier stages of this run saved. */
	readonly outputs: OutputReader;
}

export interface OutputReader {
	/** The directory outputs are kept in, for commands to read. Undefined for other stores. */
	readonly directory: string | undefined;
	/** The latest output the node's task saved, or undefined when it saved none. */
	read(node: string): unknown;
}

/**
 * The stage that most recently failed in this run, with the detail it recorded. Later tasks read
 * it as `context.failure`. Stages that passed afterward do not clear it.
 */
export interface StageFailure {
	/** The graph's node name for the stage. */
	readonly node: string;
	/** The task the stage ran. */
	readonly task: string;
	/** The stage's key, `<transitions>:<node>`. */
	readonly key: string;
	/** `fail` when the runner reported it; `error` when every attempt threw. */
	readonly outcome: 'fail' | 'error';
	readonly detail?: Detail;
}

export type Runner = (
	task: string,
	context: TaskContext
) => Promise<Outcome | RunnerReport>;

/** Points where a test can stop the process. Each label names the write it follows. */
export type Checkpoint =
	| 'task-done' // the task ran; its result is not recorded yet
	| 'result-saved' // the result is recorded; the step has not run
	| 'step-saved'; // the step ran and the snapshot is saved

/** What the engine did. Each event names only the graph's own node names and the caller's keys. */
export type RunEvent =
	| {
			readonly type: 'stage-entered';
			readonly node: string;
			readonly key: string;
	  }
	| {
			readonly type: 'attempt-failed';
			readonly task: string;
			readonly key: string;
			readonly attempt: number;
			readonly error: string;
	  }
	| {
			readonly type: 'task-settled';
			readonly task: string;
			readonly key: string;
			readonly outcome: StoredOutcome;
			/** The detail the runner gave with the outcome, if any. */
			readonly detail?: Detail | undefined;
			/** `stored`: a result from an earlier run. `verified`: found by `verify`. `ran`: this run. */
			readonly source: 'stored' | 'verified' | 'ran';
	  }
	| {
			readonly type: 'step-taken';
			readonly taken: number;
			readonly node: string;
	  }
	| {
			readonly type: 'run-started';
			/** True when the run continues from saved state, such as after a crash. */
			readonly resumed: boolean;
			/** Results loaded for the stage the run resumes at. */
			readonly storedResults: number;
	  }
	| { readonly type: 'run-stopped'; readonly result: Result };

export interface DurableOptions {
	/** Path to the compiled graph module (`<dir>/<graph>.ts`). */
	readonly modulePath: string;
	/** Used with the file store when `store` is not given. */
	readonly stateFile: string;
	readonly store?: StateStore;
	readonly runner: Runner;
	/**
	 * Asks the outside world whether a task's effect already happened for `key`. Called before a
	 * task runs that has no recorded result. Return its outcome if it did, or undefined to run it.
	 */
	readonly verify?: (
		task: string,
		key: string
	) => Promise<Outcome | undefined>;
	/**
	 * Data every task receives as `context.input`, such as the target a graph runs against. A resumed
	 * run must be given the same input: a different one is refused, so no task sees a mix of two.
	 */
	readonly input?: unknown;
	/** Per attempt. Default 60 seconds. */
	readonly taskTimeoutMs?: number;
	/** Attempts for a node flagged with the retry flag. Default 3. Other nodes get one. */
	readonly maxAttempts?: number;
	/** Transitions allowed per run, so a loop cannot run forever. Default 10 million. */
	readonly maxTransitions?: number;
	readonly onCheckpoint?: (label: Checkpoint, key: string) => void;
	/**
	 * Told what the engine does, as it happens. Use it for logs, metrics or traces. The handler must
	 * not throw: an exception stops the run.
	 */
	readonly onEvent?: (event: RunEvent) => void;
	/** Annotation whose value names a stage's task. */
	readonly taskAnnotation: string;
	/** Node flag that allows retries. */
	readonly retryFlag: string;
	/**
	 * Conditions the engine sets from a task's outcome. Required when the graph has tasks; the graph
	 * must declare both. A graph without tasks may leave this out.
	 */
	readonly outcomeConditions?:
		| {
				readonly pass: string;
				readonly fail: string;
				/**
				 * When given, a task that throws on every attempt sets this condition instead of stopping the
				 * run, so the graph can route it with `when <error>`. Without it, such a task stops the run.
				 */
				readonly error?: string;
		  }
		| undefined;
}

export type Result =
	| {
			status: 'ended';
			transitions: number;
			/** The end node the run reached. */
			node: string;
			/** The output that end node's task saved, when it saved one. */
			output?: unknown;
			/** The outcome of the end node's own stage: `none` when it has no task. */
			outcome: StoredOutcome;
	  }
	| { status: 'stalled'; transitions: number; node: string }
	| { status: 'error'; transitions: number; key: string; error: string };

/** A task that failed, after all its attempts. The run stops; engine errors are not wrapped in this. */
class TaskFailure extends Error {}

/** Clears a recorded error, so the next run runs that task again. */
export async function retryFailedTask(
	store: StateStore,
	key: string
): Promise<void> {
	const sequence = sequenceOf(key);
	await store.withLock(() => {
		if (store.loadRecords(sequence).get(key)?.outcome !== 'error') {
			throw new Error(`task "${key}" has no stored error`);
		}
		store.recordResult(key, sequence, undefined);
	});
}

/** The parts of the generated run this engine uses. */
interface RunLike {
	readonly node: string;
	readonly status: 'running' | 'ended' | 'error';
	readonly error: string | null;
	readonly transitions: number;
	set(condition: string, value: boolean): void;
	finish(): void;
	advance(): number;
	snapshot(): unknown;
}

interface Session {
	readonly graph: GraphLike;
	readonly run: RunLike;
	readonly store: StateStore;
	readonly options: DurableOptions;
	/** Results for the current stage only. Cleared when the stage is done. */
	readonly results: Map<string, StoredResult>;
	/** The most recent failed stage of this run. Saved with the snapshot. */
	lastFailure: StageFailure | undefined;
	/** A fingerprint of the run's input, or null when the run has none. Saved with the snapshot. */
	readonly inputDigest: string | null;
}

/** Graphs already checked, with the options they were checked under. Keyed by graph, so it frees itself. */
const checked = new WeakMap<object, string>();

/** Runs the graph from its saved state, or from the entry if there is none. */
export async function runDurable(options: DurableOptions): Promise<Result> {
	const maxAttempts = options.maxAttempts ?? DEFAULT_MAX_ATTEMPTS;
	if (!Number.isInteger(maxAttempts) || maxAttempts < 1) {
		throw new RangeError(
			`maxAttempts must be a positive integer, got ${maxAttempts}`
		);
	}
	const file = resolve(options.modulePath);
	const { graph } = await import(file);
	const { start, restore } = await import(
		join(dirname(file), 'graphite.ts')
	);
	// The checks depend on the graph and on these two options, so a graph checked once with the same
	// options is not walked again. A fan-out checks each item's run with the same graph.
	const checkKey = JSON.stringify([options.taskAnnotation, options.outcomeConditions ?? null]);
	if (checked.get(graph) !== checkKey) {
		if (options.outcomeConditions === undefined) {
			for (let id = 0; id < graph.data.nodeCount; id++) {
				const node = graph.nodeName(id);
				if (
					taskOf(graph, node, options.taskAnnotation) !==
					undefined
				) {
					throw new RangeError(
						`the graph has tasks (node "${node}" among them), so it needs outcomeConditions naming its pass and fail conditions`
					);
				}
			}
		} else {
			const names = [
				options.outcomeConditions.pass,
				options.outcomeConditions.fail,
			];
			if (options.outcomeConditions.error !== undefined) {
				names.push(options.outcomeConditions.error);
			}
			for (const name of names) {
				try {
					graph.conditionId(name);
				} catch {
					throw new RangeError(
						`the graph has no condition "${name}", which outcomeConditions names`
					);
				}
			}
		}
		checked.set(graph, checkKey);
	}
	const limits = {
		maxTransitions: options.maxTransitions ?? DEFAULT_MAX_TRANSITIONS,
	};
	const store = options.store ?? fileStore(options.stateFile);
	return store.withLock(async () => {
		const saved = store.loadSnapshot();
		const resumed = saved !== undefined;
		const { runSnapshot, lastFailure, inputDigest: savedDigest } =
			splitSnapshot(saved);
		const inputDigest =
			options.input === undefined ? null : digestOf(options.input);
		// A snapshot from before inputs were kept has no digest, and is accepted as it is.
		if (resumed && savedDigest !== undefined && savedDigest !== inputDigest) {
			throw new Error(
				'the input does not match the input this run started with, so the run cannot resume'
			);
		}
		const run: RunLike = resumed
			? restore(graph, runSnapshot, limits)
			: start(graph, limits);
		const results = store.loadRecords(run.transitions);
		const session: Session = {
			graph,
			run,
			store,
			options,
			results,
			lastFailure,
			inputDigest,
		};
		options.onEvent?.({
			type: 'run-started',
			resumed,
			storedResults: results.size,
		});
		for (;;) {
			const stop = await runStage(session);
			if (stop !== undefined) {
				options.onEvent?.({
					type: 'run-stopped',
					result: stop,
				});
				return stop;
			}
		}
	});
}

/** Runs the current stage. Returns the result once the run stops, or undefined to go on. */
async function runStage(session: Session): Promise<Result | undefined> {
	const { graph, run } = session;
	const node = run.node;
	const key = `${run.transitions}:${node}`;
	session.options.onEvent?.({ type: 'stage-entered', node, key });
	if (run.status === 'error') return errorResult(session, key);

	// The task of the node a run reaches is always settled, including an end node's: the run
	// ends after that task, not on arrival.
	const task = taskOf(graph, node, session.options.taskAnnotation);
	let settled: StoredResult = { outcome: 'none' };
	if (task !== undefined) {
		try {
			settled = await settleTask(session, node, task, key);
		} catch (error) {
			if (!(error instanceof TaskFailure)) throw error;
			return {
				status: 'error',
				transitions: run.transitions,
				key,
				error: error.message,
			};
		}
	}

	const outcome = settled.outcome;
	const conditions = session.options.outcomeConditions;
	if (conditions !== undefined) {
		run.set(conditions.pass, outcome === 'pass');
		run.set(conditions.fail, outcome === 'fail');
		if (conditions.error !== undefined) {
			run.set(conditions.error, outcome === 'error');
		}
	}
	if (run.status === 'ended') {
		saveSnapshot(session, 'step-saved', key);
		const output = session.store.loadOutput?.(node);
		return {
			status: 'ended',
			transitions: run.transitions,
			node,
			outcome,
			output,
		};
	}
	run.finish();
	// One transition per stage: the next node's gates must not read this stage's outcome.
	const taken = run.advance() < 0 ? 0 : 1;
	session.options.onEvent?.({ type: 'step-taken', taken, node: run.node });
	saveSnapshot(session, 'step-saved', key);
	// The next stage has a later sequence, so nothing recorded for this one is needed again.
	session.results.clear();
	if (statusOf(run) === 'error') return errorResult(session, key);
	if (taken === 0) {
		return {
			status: 'stalled',
			transitions: run.transitions,
			node: run.node,
		};
	}
	return undefined;
}

/**
 * The task's result. A recorded result is returned without running the task again. Throws when
 * the task failed, either now or in an earlier run.
 */
async function settleTask(
	session: Session,
	node: string,
	task: string,
	key: string
): Promise<StoredResult> {
	const { graph, options, results } = session;
	const stored = results.get(key);
	if (stored?.outcome === 'error') {
		noteFailure(session, node, task, key, stored);
		if (routesErrors(session)) return stored;
		throw new TaskFailure(`task ${task} failed in an earlier run`);
	}
	if (stored !== undefined) {
		noteFailure(session, node, task, key, stored);
		options.onEvent?.({
			type: 'task-settled',
			task,
			key,
			outcome: stored.outcome,
			detail: stored.detail,
			source: 'stored',
		});
		return stored;
	}

	if (options.verify !== undefined) {
		const done: unknown = await options.verify(task, key);
		if (done !== undefined) {
			if (!isOutcome(done)) {
				throw new Error(`verify returned "${String(done)}"`);
			}
			const result: StoredResult = { outcome: done };
			recordResult(session, key, result);
			noteFailure(session, node, task, key, result);
			options.onEvent?.({
				type: 'task-settled',
				task,
				key,
				outcome: done,
				source: 'verified',
			});
			return result;
		}
	}
	const attempts = graph.nodeFlags(node).includes(options.retryFlag)
		? (options.maxAttempts ?? DEFAULT_MAX_ATTEMPTS)
		: 1;
	// Only the attempt's own failure is caught here. A checkpoint or a store failure is not a task error.
	let report: RunnerReport | undefined;
	let failure = '';
	try {
		report = await attempt(
			options,
			task,
			key,
			attempts,
			session.lastFailure,
			outputsOf(session)
		);
	} catch (error) {
		failure = errorText(error);
	}
	if (report === undefined) {
		const result: StoredResult = {
			outcome: 'error',
			detail: { message: capMessage(failure) },
		};
		if (!routesErrors(session)) {
			recordResult(session, key, result);
			noteFailure(session, node, task, key, result);
			throw new TaskFailure(`task ${task} failed: ${failure}`);
		}
		return finishTask(session, node, task, key, result, undefined);
	}
	options.onCheckpoint?.('task-done', key);
	const result: StoredResult =
		report.detail === undefined
			? { outcome: report.outcome }
			: { outcome: report.outcome, detail: report.detail };
	return finishTask(session, node, task, key, result, report.output);
}

/** Saves the task's output, records its result, and reports it. Shared by every way a task settles. */
function finishTask(
	session: Session,
	node: string,
	task: string,
	key: string,
	result: StoredResult,
	output: unknown
): StoredResult {
	if (output !== undefined) {
		if (session.store.saveOutput === undefined) {
			throw new Error(
				`task ${task} returned an output, but the store cannot keep outputs`
			);
		}
		// Saved before the result, so a crash between the two reruns the task and overwrites it.
		session.store.saveOutput(node, output);
	}
	recordResult(session, key, result);
	noteFailure(session, node, task, key, result);
	session.options.onEvent?.({
		type: 'task-settled',
		task,
		key,
		outcome: result.outcome,
		detail: result.detail,
		source: 'ran',
	});
	return result;
}

/** Whether the graph routes thrown errors with its `errored` condition, rather than stopping. */
function routesErrors(session: Session): boolean {
	return session.options.outcomeConditions?.error !== undefined;
}

/** The outputs of earlier stages, read through the store. */
function outputsOf(session: Session): OutputReader {
	return {
		directory: session.store.outputDirectory,
		read: (node) => session.store.loadOutput?.(node),
	};
}

/** Makes `node` the run's last failure when the result is a failure. */
function noteFailure(
	session: Session,
	node: string,
	task: string,
	key: string,
	result: StoredResult
): void {
	if (result.outcome !== 'fail' && result.outcome !== 'error') return;
	session.lastFailure = {
		node,
		task,
		key,
		outcome: result.outcome,
		...(result.detail === undefined ? {} : { detail: result.detail }),
	};
}

/** Reads the status without narrowing: a step can change it, which the compiler cannot see. */
function statusOf(run: RunLike): RunLike['status'] {
	return run.status;
}

function errorResult(session: Session, key: string): Result {
	const error =
		session.run.error === 'limit'
			? `run reached maxTransitions (${session.options.maxTransitions ?? DEFAULT_MAX_TRANSITIONS})`
			: String(session.run.error);
	return {
		status: 'error',
		transitions: session.run.transitions,
		key,
		error,
	};
}

/**
 * Saves the run's position and the run's last failure. Results are written by recordResult, so
 * this stays small.
 */
function saveSnapshot(session: Session, label: Checkpoint, key: string): void {
	session.store.saveSnapshot({
		run: session.run.snapshot(),
		lastFailure: session.lastFailure ?? null,
		inputDigest: session.inputDigest,
	});
	session.options.onCheckpoint?.(label, key);
}

/** Records one task's result. It is durable before the run moves on. */
function recordResult(
	session: Session,
	key: string,
	result: StoredResult
): void {
	session.store.recordResult(key, session.run.transitions, result);
	session.results.set(key, result);
	session.options.onCheckpoint?.('result-saved', key);
}

/**
 * Reads the snapshot the engine saved: `{ run, lastFailure }`. A bare run snapshot, from before
 * failures were kept, has no `run` field and no last failure.
 */
function splitSnapshot(saved: unknown): {
	runSnapshot: unknown;
	lastFailure: StageFailure | undefined;
	/** The digest the run started with. Undefined for a snapshot that predates digests. */
	inputDigest: string | null | undefined;
} {
	if (!isRecord(saved) || !('run' in saved)) {
		return {
			runSnapshot: saved,
			lastFailure: undefined,
			inputDigest: undefined,
		};
	}
	return {
		runSnapshot: saved.run,
		lastFailure: parseFailure(saved.lastFailure),
		// Missing from wrappers saved before digests existed, which reads as undefined: accepted.
		inputDigest: saved.inputDigest as string | null | undefined,
	};
}

/** A stable fingerprint of a JSON input, used to refuse a resume with a different one. */
function digestOf(input: unknown): string {
	return createHash('sha256')
		.update(JSON.stringify(input) ?? 'null')
		.digest('hex');
}

function parseFailure(value: unknown): StageFailure | undefined {
	if (value === undefined || value === null) return undefined;
	if (
		!isRecord(value) ||
		typeof value.node !== 'string' ||
		typeof value.task !== 'string' ||
		typeof value.key !== 'string' ||
		(value.outcome !== 'fail' && value.outcome !== 'error')
	) {
		throw new Error('state file is malformed: lastFailure');
	}
	const detail = isDetail(value.detail) ? value.detail : undefined;
	return {
		node: value.node,
		task: value.task,
		key: value.key,
		outcome: value.outcome,
		...(detail === undefined ? {} : { detail }),
	};
}

function sequenceOf(key: string): number {
	return Number(key.slice(0, key.indexOf(':')));
}

async function attempt(
	options: DurableOptions,
	task: string,
	key: string,
	attempts: number,
	failure: StageFailure | undefined,
	outputs: OutputReader
): Promise<RunnerReport> {
	const timeoutMs = options.taskTimeoutMs ?? DEFAULT_TASK_TIMEOUT_MS;
	let lastError: unknown = new Error('no attempts');
	for (let number = 1; number <= attempts; number++) {
		try {
			return await runOnce(
				options.runner,
				task,
				{ key, attempt: number, failure, input: options.input, outputs },
				timeoutMs
			);
		} catch (error) {
			lastError = error;
			options.onEvent?.({
				type: 'attempt-failed',
				task,
				key,
				attempt: number,
				error: errorText(error),
			});
		}
	}
	throw lastError;
}

async function runOnce(
	runner: Runner,
	task: string,
	ids: {
		key: string;
		attempt: number;
		failure: StageFailure | undefined;
		input: unknown;
		outputs: OutputReader;
	},
	timeoutMs: number
): Promise<RunnerReport> {
	const controller = new AbortController();
	let timer: ReturnType<typeof setTimeout> | undefined;
	const timeout = new Promise<never>((_, reject) => {
		timer = setTimeout(() => {
			controller.abort();
			reject(new Error(`timed out after ${timeoutMs}ms`));
		}, timeoutMs);
	});
	try {
		const value: unknown = await Promise.race([
			runner(task, { ...ids, signal: controller.signal }),
			timeout,
		]);
		return normalizeReport(value);
	} finally {
		clearTimeout(timer);
	}
}

function errorText(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
