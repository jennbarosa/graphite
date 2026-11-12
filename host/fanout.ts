// Fan-out: a list graph produces a list of items, and an item graph runs once per item. Each item
// has its own state file, so a crash or a stop resumes that item where it was. Two things keep the
// host from re-running items:
// - a seen file holds the key of every item already started, so a key is never started twice;
// - `untilPassed` stops starting items once that many have ended on a passing stage.
// The list graph must end with an array as its output. Each item is a JSON object with a string
// field named by `key`, which is also the item's name on disk, so it must be a plain identifier.
import {
	closeSync,
	existsSync,
	ftruncateSync,
	fsyncSync,
	openSync,
	readFileSync,
} from 'node:fs';
import { type DurableOptions, type Result, runDurable } from './durable';
import { appendLine } from './store';

export interface FanOutOptions {
	/** The list graph's run. Its end output is the array of items. */
	readonly list: DurableOptions;
	/** The options for one item's run, given the item as its input and the item's state file. */
	readonly item: (input: unknown, stateFile: string) => DurableOptions;
	/** The state file for an item, by its key. */
	readonly itemStateFile: (key: string) => string;
	/** The seen file: the keys of items already started, kept across runs. */
	readonly seenFile: string;
	/** The field of each item that names it. */
	readonly key: string;
	/** Stop starting items once this many have ended on a passing stage. */
	readonly untilPassed?: number;
	/** Told each item's result, as it comes. */
	readonly onItem?: (key: string, result: Result) => void;
}

export type FanOutSummary =
	| { status: 'ended'; items: number; started: number; passed: number }
	| { status: 'stopped'; list: Result };

const KEY = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;

export async function fanOut(options: FanOutOptions): Promise<FanOutSummary> {
	const list = await runDurable(options.list);
	if (list.status !== 'ended') return { status: 'stopped', list };
	const items: unknown = list.output;
	if (!Array.isArray(items)) {
		throw new Error(
			'the list graph must end with an array as its output, one object per item'
		);
	}

	const seen = readSeen(options.seenFile);
	let started = 0;
	let passed = 0;
	for (const item of items) {
		if (options.untilPassed !== undefined && passed >= options.untilPassed) break;
		const key = keyOf(item, options.key);
		if (seen.has(key)) continue;
		// Recorded before the item runs, so a crash mid-item does not start it again.
		seen.add(key);
		appendLine(options.seenFile, key);
		started++;
		const result = await runDurable(
			options.item(item, options.itemStateFile(key))
		);
		if (result.status === 'ended' && result.outcome === 'pass') passed++;
		options.onItem?.(key, result);
	}
	return { status: 'ended', items: items.length, started, passed };
}

function keyOf(item: unknown, field: string): string {
	const key =
		typeof item === 'object' && item !== null
			? (item as Record<string, unknown>)[field]
			: undefined;
	if (typeof key !== 'string' || !KEY.test(key)) {
		throw new Error(
			`each item needs a string "${field}" of letters, digits, dots, dashes or underscores, got ${JSON.stringify(key)}`
		);
	}
	return key;
}

/**
 * The keys in the seen file, one per line. A last line cut short by a crash is dropped, and the file
 * is cut back to the end of the last complete line, so the next append starts on a clean line.
 */
function readSeen(file: string): Set<string> {
	if (!existsSync(file)) return new Set();
	const bytes = readFileSync(file);
	const completeEnd = bytes.lastIndexOf(0x0a) + 1;
	if (completeEnd < bytes.length) {
		const fd = openSync(file, 'r+');
		try {
			ftruncateSync(fd, completeEnd);
			fsyncSync(fd);
		} finally {
			closeSync(fd);
		}
	}
	const lines = bytes.subarray(0, completeEnd).toString('utf8').split('\n');
	// The last piece is what follows the final newline, which is always empty now.
	lines.pop();
	for (const line of lines) {
		if (!KEY.test(line)) {
			throw new Error(`seen file ${file} has a line that is not an item key: ${line}`);
		}
	}
	return new Set(lines);
}
