// Durable host for a compiled graph. It runs each stage's task through a runner, saves state
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
import {
	closeSync,
	existsSync,
	fstatSync,
	fsyncSync,
	mkdirSync,
	ftruncateSync,
	openSync,
	readFileSync,
	readSync,
	renameSync,
	unlinkSync,
	writeFileSync,
	writeSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';

const DEFAULT_MAX_TRANSITIONS = 10_000_000;
const DEFAULT_MAX_ATTEMPTS = 3;
const DEFAULT_TASK_TIMEOUT_MS = 60_000;
const READ_CHUNK_BYTES = 64 * 1024;
/** Largest stage output, as UTF-8 JSON. */
export const MAX_OUTPUT_BYTES = 1_048_576;
/** Longest `detail.message` kept. A longer message is cut and ends with an ellipsis. */
export const MAX_DETAIL_MESSAGE = 1_000;
/** Longest `detail.code`. */
export const MAX_DETAIL_CODE = 64;
/** Largest `detail.data`, as UTF-8 JSON. */
export const MAX_DETAIL_DATA_BYTES = 4_096;

const OUTCOMES = ['pass', 'fail', 'none'] as const;
export type Outcome = (typeof OUTCOMES)[number];
/** A finished task's recorded result: its outcome, or `error` when every attempt threw. */
export type StoredOutcome = Outcome | 'error';
const STORED_OUTCOMES: readonly string[] = [...OUTCOMES, 'error'];

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

/**
 * Optional context a runner reports with its outcome. The host stores it with the result, writes it
 * to the run log, and passes it to later tasks as `context.failure` when the stage failed.
 */
export interface Detail {
	/** What happened, in a sentence. Longer text is cut to `MAX_DETAIL_MESSAGE` characters. */
	readonly message: string;
	/** A short label to match on, such as `timeout`. At most `MAX_DETAIL_CODE` characters. */
	readonly code?: string;
	/** JSON for a later task, such as the names of failing checks. At most `MAX_DETAIL_DATA_BYTES`. */
	readonly data?: unknown;
}

/** What a runner returns: a bare outcome, or an outcome with its detail. */
export type RunnerReport = {
	readonly outcome: Outcome;
	readonly detail?: Detail;
	/** JSON a later task reads with `context.outputs.read(node)`. At most `MAX_OUTPUT_BYTES`. */
	readonly output?: unknown;
};

/** Reads the outputs of earlier stages in this run, by node name. */
export interface OutputReader {
	/** The directory outputs are kept in, for commands to read. Undefined for other stores. */
	readonly directory: string | undefined;
	/** The latest output the node's task saved, or undefined when it saved none. */
	read(node: string): Promise<unknown>;
}

/** A finished task's recorded result: its outcome, and the detail the runner gave, if any. */
export interface StoredResult {
	readonly outcome: StoredOutcome;
	readonly detail?: Detail;
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

/** What the host did. Each event names only the graph's own node names and the caller's keys. */
export type HostEvent =
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
	 * Told what the host does, as it happens. Use it for logs, metrics or traces. The handler must
	 * not throw: an exception stops the run.
	 */
	readonly onEvent?: (event: HostEvent) => void;
	/** Annotation whose value names a stage's task. */
	readonly taskAnnotation: string;
	/** Node flag that allows retries. */
	readonly retryFlag: string;
	/**
	 * Conditions the host sets from a task's outcome. Required when the graph has tasks; the graph
	 * must declare both. A graph without tasks may leave this out.
	 */
	readonly outcomeConditions?: {
		readonly pass: string;
		readonly fail: string;
		/**
		 * When given, a task that throws on every attempt sets this condition instead of stopping the
		 * run, so the graph can route it with `when <error>`. Without it, such a task stops the run.
		 */
		readonly error?: string;
	};
}

export type Result =
	| {
			status: 'ended';
			transitions: number;
			/** The end node the run reached. */
			node: string;
			/** The output that end node's task saved, when it saved one. */
			output?: unknown;
	  }
	| { status: 'stalled'; transitions: number; node: string }
	| { status: 'error'; transitions: number; key: string; error: string };

/** A task that failed, after all its attempts. The run stops; host errors are not wrapped in this. */
class TaskFailure extends Error {}

/**
 * Where a run's state lives. `saveSnapshot` must be atomic: a reader sees the old snapshot or the
 * new one. `recordResult` must be durable when it returns. `loadRecords` returns the last outcome
 * recorded for each key at `since` or later, and leaves out keys whose last record cleared them.
 * `recordResult` with `undefined` clears a result. `withLock` must exclude other holders of the
 * same state while `body` runs.
 */
export interface StateStore {
	/** Where stage outputs are kept, when the store keeps them as files that commands can read. */
	readonly outputDirectory?: string;
	/** Keeps a stage's output, so later tasks can read it. Optional: a store without it cannot. */
	saveOutput?(node: string, value: unknown): void;
	/** The latest output saved for a node, or undefined. */
	loadOutput?(node: string): unknown;
	loadSnapshot(): unknown;
	loadRecords(since: number): Map<string, StoredResult>;
	saveSnapshot(snapshot: unknown): void;
	recordResult(
		key: string,
		sequence: number,
		result: StoredResult | undefined
	): void;
	withLock<T>(body: () => T | Promise<T>): Promise<T>;
}

/**
 * The state store backed by local files. `<file>` holds the snapshot, rewritten atomically at
 * each step. `<file>.results` holds one JSON line per recorded result, appended and fsynced as it
 * is recorded. `<file>.lock` lets one process at a time use the state.
 */
export function fileStore(stateFile: string): StateStore {
	const resultsFile = `${stateFile}.results`;
	const outputDir = `${stateFile}.outputs`;
	return {
		outputDirectory: outputDir,
		saveOutput: (node, value) => {
			mkdirSync(outputDir, { recursive: true });
			writeAtomic(join(outputDir, `${node}.json`), JSON.stringify(value));
		},
		loadOutput: (node) => {
			const file = join(outputDir, `${node}.json`);
			return existsSync(file)
				? JSON.parse(readFileSync(file, 'utf8'))
				: undefined;
		},
		loadSnapshot: () =>
			existsSync(stateFile) ? readSnapshot(stateFile) : undefined,
		loadRecords: (since) => readRecords(resultsFile, since),
		saveSnapshot: (snapshot) =>
			writeAtomic(stateFile, JSON.stringify({ snapshot })),
		recordResult: (key, sequence, result) =>
			appendLine(
				resultsFile,
				JSON.stringify({
					key,
					sequence,
					outcome: result?.outcome ?? null,
					detail: result?.detail,
				})
			),
		withLock: (body) => withLock(stateFile, body),
	};
}

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

/** The parts of the generated graph module this host uses. */
interface GraphLike {
	readonly data: { readonly nodeCount: number };
	nodeName(id: number): string;
	nodeAnnotations(node: string): readonly { key: string; value?: string }[];
	nodeFlags(node: string): readonly string[];
	conditionId(name: string): number;
}

/** The parts of the generated run this host uses. */
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
			...(output === undefined ? {} : { output }),
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
	let report: RunnerReport;
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
		const result: StoredResult = {
			outcome: 'error',
			detail: { message: capMessage(errorText(error)) },
		};
		recordResult(session, key, result);
		noteFailure(session, node, task, key, result);
		if (routesErrors(session)) {
			options.onEvent?.({
				type: 'task-settled',
				task,
				key,
				outcome: 'error',
				detail: result.detail,
				source: 'ran',
			});
			return result;
		}
		throw new TaskFailure(`task ${task} failed: ${errorText(error)}`);
	}
	options.onCheckpoint?.('task-done', key);
	const result: StoredResult =
		report.detail === undefined
			? { outcome: report.outcome }
			: { outcome: report.outcome, detail: report.detail };
	if (report.output !== undefined) {
		if (session.store.saveOutput === undefined) {
			throw new Error(
				`task ${task} returned an output, but the store cannot keep outputs`
			);
		}
		// Saved before the result, so a crash between the two reruns the task and overwrites it.
		session.store.saveOutput(node, report.output);
	}
	recordResult(session, key, result);
	noteFailure(session, node, task, key, result);
	options.onEvent?.({
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
		read: async (node) => session.store.loadOutput?.(node),
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
 * Reads the snapshot the host saved: `{ run, lastFailure }`. A bare run snapshot, from before
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
	const digest = saved.inputDigest;
	return {
		runSnapshot: saved.run,
		lastFailure: parseFailure(saved.lastFailure),
		inputDigest: typeof digest === 'string' ? digest : null,
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

function isDetail(value: unknown): value is Detail {
	return isRecord(value) && typeof value.message === 'string';
}

function sequenceOf(key: string): number {
	return Number(key.slice(0, key.indexOf(':')));
}

function taskOf(
	graph: GraphLike,
	node: string,
	annotation: string
): string | undefined {
	return graph
		.nodeAnnotations(node)
		.find((entry) => entry.key === annotation)?.value;
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

/** Checks what a runner returned, and returns it as a report. Throws when it is not valid. */
function normalizeReport(value: unknown): RunnerReport {
	if (isOutcome(value)) return { outcome: value };
	if (!isRecord(value)) {
		throw new Error(`runner returned "${String(value)}"`);
	}
	if (!isOutcome(value.outcome)) {
		throw new Error(
			`runner returned an object whose outcome is "${String(value.outcome)}"`
		);
	}
	const report: { outcome: Outcome; detail?: Detail; output?: unknown } = {
		outcome: value.outcome,
	};
	if (value.detail !== undefined) report.detail = normalizeDetail(value.detail);
	if (value.output !== undefined) report.output = normalizeOutput(value.output);
	return report;
}

/** A copy of a stage output that is JSON and within the size limit. */
function normalizeOutput(value: unknown): unknown {
	const text = JSON.stringify(value);
	if (text === undefined || Buffer.byteLength(text) > MAX_OUTPUT_BYTES) {
		throw new Error(
			`runner output must be JSON of at most ${MAX_OUTPUT_BYTES} bytes`
		);
	}
	return JSON.parse(text) as unknown;
}

/** Checks a runner's detail against the limits, and returns a copy the host can store. */
function normalizeDetail(value: unknown): Detail {
	if (!isRecord(value) || typeof value.message !== 'string') {
		throw new Error('runner detail needs a string message');
	}
	const detail: { message: string; code?: string; data?: unknown } = {
		message: capMessage(value.message),
	};
	if (value.code !== undefined) {
		if (
			typeof value.code !== 'string' ||
			value.code.length > MAX_DETAIL_CODE
		) {
			throw new Error(
				`runner detail code must be a string of at most ${MAX_DETAIL_CODE} characters`
			);
		}
		detail.code = value.code;
	}
	if (value.data !== undefined) {
		const text = JSON.stringify(value.data);
		if (text === undefined || Buffer.byteLength(text) > MAX_DETAIL_DATA_BYTES) {
			throw new Error(
				`runner detail data must be JSON of at most ${MAX_DETAIL_DATA_BYTES} bytes`
			);
		}
		detail.data = JSON.parse(text) as unknown;
	}
	return detail;
}

/** The message, cut to `MAX_DETAIL_MESSAGE` characters with an ellipsis when it is longer. */
function capMessage(text: string): string {
	return text.length <= MAX_DETAIL_MESSAGE
		? text
		: `${text.slice(0, MAX_DETAIL_MESSAGE - 1)}…`;
}

function isOutcome(value: unknown): value is Outcome {
	return (
		typeof value === 'string' &&
		(OUTCOMES as readonly string[]).includes(value)
	);
}

function isStoredOutcome(value: unknown): value is StoredOutcome {
	return typeof value === 'string' && STORED_OUTCOMES.includes(value);
}

function errorText(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function readSnapshot(file: string): unknown {
	const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'));
	if (!isRecord(parsed) || !('snapshot' in parsed)) {
		throw new Error(`state file ${file} is malformed`);
	}
	return parsed.snapshot;
}

interface ResultRecord {
	readonly key: string;
	readonly sequence: number;
	readonly outcome: StoredOutcome | null;
	readonly detail?: Detail | undefined;
}

function parseRecord(line: string): ResultRecord | undefined {
	try {
		const value: unknown = JSON.parse(line);
		if (
			!isRecord(value) ||
			typeof value.key !== 'string' ||
			typeof value.sequence !== 'number'
		) {
			return undefined;
		}
		const outcome = value.outcome;
		if (outcome !== null && !isStoredOutcome(outcome)) {
			return undefined;
		}
		if (value.detail !== undefined && !isDetail(value.detail)) {
			return undefined;
		}
		return {
			key: value.key,
			sequence: value.sequence,
			outcome,
			detail: value.detail,
		};
	} catch {
		return undefined;
	}
}

/**
 * Reads the records at `since` or later, in order, keeping the last result for each key. The
 * file is read in chunks, so only those records are held in memory. A last line cut short by a
 * crash is dropped, and the file is cut back to the end of the last complete line, so later
 * appends start on a clean line. A bad complete line means the file is damaged.
 */
function readRecords(file: string, since: number): Map<string, StoredResult> {
	const latest = new Map<string, StoredResult | null>();
	if (!existsSync(file)) return new Map();
	const fd = openSync(file, 'r+');
	try {
		const size = fstatSync(fd).size;
		const completeEnd = forEachLine(fd, (line) => {
			if (line === '') return;
			const record = parseRecord(line);
			if (record === undefined) {
				throw new Error(`results file ${file} is malformed`);
			}
			if (record.sequence >= since) {
				latest.set(
					record.key,
					record.outcome === null
						? null
						: record.detail === undefined
							? { outcome: record.outcome }
							: { outcome: record.outcome, detail: record.detail }
				);
			}
		});
		if (completeEnd < size) {
			ftruncateSync(fd, completeEnd);
			fsyncSync(fd);
		}
	} finally {
		closeSync(fd);
	}
	const recovered = new Map<string, StoredResult>();
	for (const [key, result] of latest) {
		if (result !== null) recovered.set(key, result);
	}
	return recovered;
}

/**
 * Calls `visit` with each complete line, in order, reading the file in chunks. Returns the byte
 * offset just past the last complete line. Text after it is an unfinished line.
 */
function forEachLine(fd: number, visit: (line: string) => void): number {
	const chunk = Buffer.alloc(READ_CHUNK_BYTES);
	let carry = Buffer.alloc(0);
	let offset = 0;
	let completeEnd = 0;
	for (;;) {
		const bytes = readSync(fd, chunk, 0, READ_CHUNK_BYTES, offset);
		if (bytes === 0) break;
		const data = Buffer.concat([carry, chunk.subarray(0, bytes)]);
		const dataStart = offset - carry.length;
		offset += bytes;
		let start = 0;
		let newline = data.indexOf(0x0a);
		while (newline !== -1) {
			visit(data.subarray(start, newline).toString('utf8'));
			start = newline + 1;
			completeEnd = dataStart + start;
			newline = data.indexOf(0x0a, start);
		}
		carry = Buffer.from(data.subarray(start));
	}
	return completeEnd;
}

/** Appends one line and makes it durable before returning. */
function appendLine(file: string, line: string): void {
	const isNew = !existsSync(file);
	const fd = openSync(file, 'a');
	try {
		writeFileSync(fd, `${line}\n`);
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
	if (isNew) fsyncDir(dirname(file));
}

/** Temp file, fsync, rename, then fsync the directory so the rename itself is durable. */
function writeAtomic(file: string, text: string): void {
	const temp = `${file}.${process.pid}.tmp`;
	const fd = openSync(temp, 'w');
	try {
		writeFileSync(fd, text);
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
	renameSync(temp, file);
	fsyncDir(dirname(file));
}

function fsyncDir(dir: string): void {
	const fd = openSync(dir, 'r');
	try {
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
}

async function withLock<T>(
	stateFile: string,
	body: () => T | Promise<T>
): Promise<T> {
	const release = acquire(stateFile);
	try {
		return await body();
	} finally {
		release();
	}
}

/**
 * Creates `<state>.lock` holding this process's pid. A lock whose pid is no longer running is
 * taken over. shortcut: the takeover is not atomic, so two operators recovering at the same
 * instant could both win; upgrade to flock if that happens.
 */
function acquire(stateFile: string): () => void {
	const lock = `${stateFile}.lock`;
	for (let tries = 0; tries < 2; tries++) {
		try {
			const fd = openSync(lock, 'wx');
			writeSync(fd, String(process.pid));
			closeSync(fd);
			return () => unlinkSync(lock);
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== 'EEXIST')
				throw error;
			const owner = Number(readFileSync(lock, 'utf8'));
			if (alive(owner)) {
				throw new Error(
					`state ${stateFile} is locked by process ${owner}`
				);
			}
			unlinkSync(lock);
		}
	}
	throw new Error(`could not lock ${stateFile}`);
}

function alive(pid: number): boolean {
	if (!Number.isInteger(pid) || pid <= 0) return false;
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		return (error as NodeJS.ErrnoException).code === 'EPERM';
	}
}
