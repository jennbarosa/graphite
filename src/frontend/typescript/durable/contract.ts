// The contract the durable engine shares with its runners and state stores: outcomes, the detail a
// runner may report, a stored result, the state store interface, and the detail limits. Types and
// constants only.

/** Longest `detail.message` kept. A longer message is cut and ends with an ellipsis. */
export const MAX_DETAIL_MESSAGE = 1_000;
/** Longest `detail.code`. */
export const MAX_DETAIL_CODE = 64;
/** Largest `detail.data`, as UTF-8 JSON. */
export const MAX_DETAIL_DATA_BYTES = 4_096;

export const OUTCOMES = ['pass', 'fail', 'none'] as const;
export type Outcome = (typeof OUTCOMES)[number];
/** A finished task's recorded result: its outcome, or `error` when every attempt threw. */
export type StoredOutcome = Outcome | 'error';

/**
 * Optional context a runner reports with its outcome. The engine stores it with the result, writes it
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

/** A finished task's recorded result: its outcome, and the detail the runner gave, if any. */
export interface StoredResult {
	readonly outcome: StoredOutcome;
	readonly detail?: Detail;
}

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
