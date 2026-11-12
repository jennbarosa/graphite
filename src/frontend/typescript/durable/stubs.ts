// The parts of `graphite debug --simulate` that do no I/O: parsing the stubs, answering each task
// call from its stub, and turning the engine's events and result into the lines a simulation prints.
import type { Result, RunEvent, Runner } from './engine';

export const STUB_OUTCOMES = ['pass', 'fail', 'none', 'throw'] as const;
export type StubOutcome = (typeof STUB_OUTCOMES)[number];

/** Parses `<task>=<outcomes>` stubs. Throws naming the first one that is not valid, or not a task of the graph. */
export function parseStubs(
	specs: readonly string[],
	tasks: readonly string[]
): Map<string, StubOutcome[]> {
	const stubs = new Map<string, StubOutcome[]>();
	for (const spec of specs) {
		const [task, outcomes] = parseStub(spec);
		if (!tasks.includes(task)) {
			throw new Error(
				`no task "${task}" in the graph; its tasks are ${tasks.join(', ') || 'none'}`
			);
		}
		if (stubs.has(task)) throw new Error(`task "${task}" is stubbed twice`);
		stubs.set(task, outcomes);
	}
	return stubs;
}

function parseStub(spec: string): [string, StubOutcome[]] {
	const at = spec.indexOf('=');
	const invalid = () =>
		new Error(
			`stub "${spec}" must be <task>=<outcomes>, outcomes from ${STUB_OUTCOMES.join(', ')}`
		);
	if (at === -1) throw invalid();
	const outcomes: StubOutcome[] = [];
	for (const name of spec.slice(at + 1).split(',')) {
		if (!isStubOutcome(name)) throw invalid();
		outcomes.push(name);
	}
	return [spec.slice(0, at), outcomes];
}

function isStubOutcome(value: string): value is StubOutcome {
	return (STUB_OUTCOMES as readonly string[]).includes(value);
}

/** A runner that answers each call of a task from its stub, and repeats the last outcome. Tasks with no stub pass. */
export function stubRunner(stubs: ReadonlyMap<string, readonly StubOutcome[]>): Runner {
	const calls = new Map<string, number>();
	return async (task) => {
		const listed = stubs.get(task) ?? ['pass'];
		const call = calls.get(task) ?? 0;
		calls.set(task, call + 1);
		const outcome = listed[Math.min(call, listed.length - 1)];
		if (outcome === 'throw') throw new Error(`simulated throw from ${task}`);
		return outcome;
	};
}

/** The line that shows one engine event, or undefined for events the simulation does not show. */
export function eventLine(event: RunEvent): string | undefined {
	switch (event.type) {
		case 'stage-entered':
			return `enter ${event.node}`;
		case 'attempt-failed':
			return `  ${event.task} attempt ${event.attempt} threw: ${event.error}`;
		case 'task-settled':
			return `  ${event.task} -> ${event.outcome}`;
		default:
			return undefined;
	}
}

/** The last line of a simulation, and whether the run ended. */
export function resultLine(result: Result): { line: string; ended: boolean } {
	switch (result.status) {
		case 'ended':
			return {
				line: `ended at ${result.node}, outcome ${result.outcome}, after ${result.transitions} transitions`,
				ended: true,
			};
		case 'stalled':
			return {
				line: `stalled at ${result.node}: no edge from it is taken`,
				ended: false,
			};
		case 'error':
			return {
				line: `error at ${result.key}: ${result.error}, after ${result.transitions} transitions`,
				ended: false,
			};
	}
}

/**
 * Passes lines to `write` until `max` have gone out. Then it writes one note that more were left
 * out, and drops the rest, so a run that loops does not flood the terminal.
 */
export function lineLimit(
	max: number,
	write: (line: string) => void
): (line: string) => void {
	let written = 0;
	let omitted = false;
	return (line) => {
		if (written < max) {
			written++;
			write(line);
		} else if (!omitted) {
			omitted = true;
			write(`... more events not shown (after ${max} lines)`);
		}
	};
}
