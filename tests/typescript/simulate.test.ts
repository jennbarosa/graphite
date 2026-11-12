// The pure parts of `debug --simulate`: the stubs, the answers they give, and the lines printed.
import { describe, expect, test } from 'bun:test';
import type { Result, Runner, RunEvent } from '../../src/frontend/typescript/durable/engine';
import {
	eventLine,
	lineLimit,
	parseStubs,
	resultLine,
	stubRunner,
} from '../../src/frontend/typescript/durable/stubs';

const noContext = {} as Parameters<Runner>[1];

describe('parseStubs', () => {
	test('maps each task to its outcomes, in order', () => {
		const stubs = parseStubs(['write=pass', 'lint=fail,pass'], ['write', 'lint', 'fix']);
		expect(stubs.get('write')).toEqual(['pass']);
		expect(stubs.get('lint')).toEqual(['fail', 'pass']);
		expect(stubs.has('fix')).toBe(false);
	});

	test('accepts every outcome, including throw', () => {
		const stubs = parseStubs(['t=pass,fail,none,throw'], ['t']);
		expect(stubs.get('t')).toEqual(['pass', 'fail', 'none', 'throw']);
	});

	test('refuses a stub without an equals sign, and one with an empty or unknown outcome', () => {
		expect(() => parseStubs(['write'], ['write'])).toThrow('must be <task>=<outcomes>');
		expect(() => parseStubs(['write='], ['write'])).toThrow('must be <task>=<outcomes>');
		expect(() => parseStubs(['write=maybe'], ['write'])).toThrow('must be <task>=<outcomes>');
		expect(() => parseStubs(['write=pass,'], ['write'])).toThrow('must be <task>=<outcomes>');
	});

	test('names the tasks the graph has when a stub names another', () => {
		expect(() => parseStubs(['nope=pass'], ['write', 'lint'])).toThrow(
			'no task "nope" in the graph; its tasks are write, lint'
		);
		expect(() => parseStubs(['nope=pass'], [])).toThrow('its tasks are none');
	});

	test('refuses a task stubbed twice', () => {
		expect(() => parseStubs(['lint=pass', 'lint=fail'], ['lint'])).toThrow(
			'task "lint" is stubbed twice'
		);
	});
});

describe('stubRunner', () => {
	test('answers each call from the list, then repeats the last outcome', async () => {
		const runner = stubRunner(new Map([['lint', ['fail', 'none']]]));
		expect(await runner('lint', noContext)).toBe('fail');
		expect(await runner('lint', noContext)).toBe('none');
		expect(await runner('lint', noContext)).toBe('none');
	});

	test('a task with no stub passes', async () => {
		expect(await stubRunner(new Map())('write', noContext)).toBe('pass');
	});

	test('throw makes the call throw, naming the task', async () => {
		const runner = stubRunner(new Map([['lint', ['throw']]]));
		await expect(runner('lint', noContext)).rejects.toThrow('simulated throw from lint');
	});
});

describe('eventLine', () => {
	test('shows the stages entered, the attempts that threw, and the outcomes', () => {
		expect(eventLine({ type: 'stage-entered', node: 'ship', key: '4:ship' })).toBe('enter ship');
		expect(
			eventLine({
				type: 'attempt-failed',
				task: 'lint',
				key: '2:check',
				attempt: 2,
				error: 'boom',
			})
		).toBe('  lint attempt 2 threw: boom');
		expect(
			eventLine({
				type: 'task-settled',
				task: 'lint',
				key: '2:check',
				outcome: 'fail',
				source: 'ran',
			})
		).toBe('  lint -> fail');
	});

	test('leaves out the events the simulation does not show', () => {
		const started: RunEvent = { type: 'run-started', resumed: false, storedResults: 0 };
		expect(eventLine(started)).toBeUndefined();
	});
});

describe('resultLine', () => {
	test('says where an ended run stopped, with its outcome and transitions', () => {
		const result: Result = { status: 'ended', transitions: 4, node: 'ship', outcome: 'pass' };
		expect(resultLine(result)).toEqual({
			line: 'ended at ship, outcome pass, after 4 transitions',
			ended: true,
		});
	});

	test('names the node a stall stopped at, and an error with its key', () => {
		expect(resultLine({ status: 'stalled', transitions: 1, node: 'a' })).toEqual({
			line: 'stalled at a: no edge from it is taken',
			ended: false,
		});
		expect(
			resultLine({ status: 'error', transitions: 3, key: '3:fix', error: 'loop' })
		).toEqual({ line: 'error at 3:fix: loop, after 3 transitions', ended: false });
	});
});

describe('lineLimit', () => {
	test('writes every line when there are no more than the limit', () => {
		const written: string[] = [];
		const emit = lineLimit(3, (line) => written.push(line));
		emit('a');
		emit('b');
		expect(written).toEqual(['a', 'b']);
	});

	test('writes one note after the limit, and drops the rest', () => {
		const written: string[] = [];
		const emit = lineLimit(2, (line) => written.push(line));
		for (const line of ['a', 'b', 'c', 'd']) emit(line);
		expect(written).toEqual(['a', 'b', '... more events not shown (after 2 lines)']);
	});
});
