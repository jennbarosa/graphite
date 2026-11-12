// Checks what a runner reports against the limits in contract.ts, and returns a copy the host can
// store. Anything invalid throws, which fails that attempt.

import {
	type Detail,
	MAX_DETAIL_CODE,
	MAX_DETAIL_DATA_BYTES,
	MAX_DETAIL_MESSAGE,
	OUTCOMES,
	type Outcome,
	type RunnerReport,
	type StoredOutcome,
} from './contract';

/** Largest stage output, as UTF-8 JSON. */
const MAX_OUTPUT_BYTES = 1_048_576;

const STORED_OUTCOMES: readonly string[] = [...OUTCOMES, 'error'];

export function isDetail(value: unknown): value is Detail {
	return isRecord(value) && typeof value.message === 'string';
}

/** Checks what a runner returned, and returns it as a report. Throws when it is not valid. */
export function normalizeReport(value: unknown): RunnerReport {
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
export function capMessage(text: string): string {
	return text.length <= MAX_DETAIL_MESSAGE
		? text
		: `${text.slice(0, MAX_DETAIL_MESSAGE - 1)}…`;
}

export function isOutcome(value: unknown): value is Outcome {
	return (
		typeof value === 'string' &&
		(OUTCOMES as readonly string[]).includes(value)
	);
}

export function isStoredOutcome(value: unknown): value is StoredOutcome {
	return typeof value === 'string' && STORED_OUTCOMES.includes(value);
}

export function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}
