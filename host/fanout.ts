// Fan-out: a list graph produces a list of items, and an item graph runs once per item. Each item
// has its own state file, so a crash or a stop resumes that item where it was. Two things keep the
// host from re-running items:
// - a seen file holds the key of every item already started, so a key is never started twice;
// - `untilPassed` stops starting items once that many have ended on a passing stage.
// The list graph must end with an array as its output. Each item is a JSON object with a string
// field named by `key`, which is also the item's name on disk, so it must be a plain identifier.
import { existsSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { type DurableOptions, type Result, runDurable } from './durable';

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
		writeSeen(options.seenFile, seen);
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

function readSeen(file: string): Set<string> {
	if (!existsSync(file)) return new Set();
	const value: unknown = JSON.parse(readFileSync(file, 'utf8'));
	if (!Array.isArray(value) || !value.every((key) => typeof key === 'string')) {
		throw new Error(`seen file ${file} must be a JSON array of item keys`);
	}
	return new Set(value as string[]);
}

/** Writes the seen file atomically, so a crash leaves the old or the new list, never half of one. */
function writeSeen(file: string, seen: Set<string>): void {
	const temp = `${file}.${process.pid}.tmp`;
	writeFileSync(temp, JSON.stringify([...seen]));
	renameSync(temp, file);
}
