// Command-backed tasks: the request on stdin, the report on stdout, and the failure modes of a
// command. Runs the fixture in tests/fixtures/commands/report.ts through `commandRunner`.
import { describe, expect, test } from 'bun:test';
import { resolve } from 'node:path';
import { commandRunner } from '../../host/commands';
import type { TaskContext } from '../../host/durable';

const fixtures = resolve(import.meta.dir, '../fixtures/commands');
const script = resolve(fixtures, 'report.ts');

/** A runner whose `lint` task runs the fixture in `mode`. */
function runnerFor(mode: string) {
	return commandRunner({
		cwd: fixtures,
		commands: { lint: [process.execPath, script, mode] },
	});
}

/** A context for one call, with the given overrides. */
function context(over: Partial<TaskContext> = {}): TaskContext {
	return {
		key: '1:check',
		attempt: 1,
		signal: new AbortController().signal,
		failure: undefined,
		input: undefined,
		outputs: { directory: undefined, read: async () => undefined },
		...over,
	};
}

describe('command-backed tasks', () => {
	test('the command receives the task, key, attempt, input and last failure on stdin', async () => {
		const failure = {
			node: 'check',
			task: 'lint',
			key: '1:check',
			outcome: 'fail' as const,
			detail: { message: 'earlier' },
		};
		const report = await runnerFor('echo')(
			'lint',
			context({ attempt: 2, input: { target: 'prospect-42' }, failure })
		);
		expect(report).toEqual({
			outcome: 'pass',
			detail: {
				message: 'echo',
				data: {
					task: 'lint',
					key: '1:check',
					attempt: 2,
					input: { target: 'prospect-42' },
					failure,
					outputs: null,
				},
			},
		});
	});

	test('a command with no input sends null, and no failure sends null', async () => {
		const report = await runnerFor('echo')('lint', context());
		expect(report).toMatchObject({
			detail: { data: { input: null, failure: null } },
		});
	});

	test('empty output with exit status 0 is a pass', async () => {
		expect(await runnerFor('quiet')('lint', context())).toEqual({ outcome: 'pass' });
	});

	test('a JSON report with a fail outcome and detail is returned as written', async () => {
		expect(await runnerFor('fail')('lint', context())).toEqual({
			outcome: 'fail',
			detail: { message: 'bad', code: 'x' },
		});
	});

	test('a nonzero exit status is a thrown attempt, naming the status', async () => {
		await expect(runnerFor('exit')('lint', context())).rejects.toThrow(
			'exited with status 3'
		);
	});

	test('output that is not JSON is a thrown attempt', async () => {
		await expect(runnerFor('junk')('lint', context())).rejects.toThrow();
	});

	test('an aborted command is stopped, and the call rejects', async () => {
		const controller = new AbortController();
		const call = runnerFor('hang')('lint', context({ signal: controller.signal }));
		setTimeout(() => controller.abort(), 100);
		await expect(call).rejects.toThrow('was stopped after its timeout');
	});

	test('a task with no command is an error naming the task', async () => {
		await expect(
			runnerFor('quiet')('write', context())
		).rejects.toThrow('no command for task write');
	});
});
