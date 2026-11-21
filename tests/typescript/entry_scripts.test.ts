// The command-line entry scripts, run in-process: each one reads `process.argv`, runs its work, and
// sets `process.exitCode`. The graphs come from the Rust driver (typescript_entry_scripts.rs).
import { describe, expect, spyOn, test } from 'bun:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

function required(name: string): string {
	const value = process.env[name];
	if (value === undefined) throw new Error(`${name} must name a compiled fixture`);
	return value;
}

const workflow = required('GRAPHITE_ENTRY_WORKFLOW');
const walk = required('GRAPHITE_ENTRY_WALK');
const list = required('GRAPHITE_ENTRY_LIST');
const item = required('GRAPHITE_ENTRY_ITEM');
const runner = required('GRAPHITE_ENTRY_RUNNER');
const workDir = required('GRAPHITE_ENTRY_DIR');

const durableCli = resolve(import.meta.dir, '../../src/frontend/typescript/durable/cli.ts');
const simulate = resolve(import.meta.dir, '../../src/frontend/typescript/durable/simulate.ts');
const debugCli = resolve(import.meta.dir, '../../cli/debug.ts');

let loads = 0;

// Bun keeps an exit status as a number from 0 to 255 and cannot unset it, so a run starts from a
// sentinel that no script exits with: 100 means the script has not finished yet.
const UNFINISHED = 100;

/** Polls until the script has set its exit status, which it does when its work is done. */
async function exitStatus(): Promise<number> {
	for (let attempt = 0; attempt < 12_000; attempt++) {
		if (process.exitCode !== UNFINISHED) return process.exitCode as number;
		await Bun.sleep(5);
	}
	throw new Error('the script did not finish');
}

/** Runs `script` with `argv`, and returns its exit status and what it printed. */
async function runEntry(
	script: string,
	argv: string[]
): Promise<{ code: number; out: string; err: string }> {
	const out: string[] = [];
	const err: string[] = [];
	const log = spyOn(console, 'log').mockImplementation((...parts: unknown[]) => {
		out.push(parts.join(' '));
	});
	const error = spyOn(console, 'error').mockImplementation((...parts: unknown[]) => {
		err.push(parts.join(' '));
	});
	const savedArgv = process.argv;
	process.exitCode = UNFINISHED;
	process.argv = ['bun', script, ...argv];
	try {
		// A query string makes each import a new module, so the entry code runs again.
		await import(`${script}?run=${loads++}`);
		const code = await exitStatus();
		return { code, out: out.join('\n'), err: err.join('\n') };
	} finally {
		process.argv = savedArgv;
		process.exitCode = 0;
		log.mockRestore();
		error.mockRestore();
	}
}

/** A runner module that emits the list items for `emit`, and passes every other task. */
function fanOutRunner(): string {
	const path = join(workDir, 'fan-runner.ts');
	const items = [{ domain: 'a.test' }, { domain: 'b.test' }];
	writeFileSync(
		path,
		`export default async function (task: string) {\n  return task === 'emit'\n    ? { outcome: 'pass', output: ${JSON.stringify(items)} }\n    : { outcome: 'pass' };\n}\n`
	);
	return path;
}

function stateFile(name: string): string {
	return join(mkdtempSync(join(tmpdir(), `graphite-${name}-`)), 'state.json');
}

describe('durable cli', () => {
	test('prints the usage and exits 2 without a graph', async () => {
		const result = await runEntry(durableCli, []);
		expect(result.code).toBe(2);
		expect(result.err).toContain('usage: cli.ts <graph.ts> <state.json>');
	});

	test('refuses --runner and --tasks together', async () => {
		const result = await runEntry(durableCli, [
			workflow,
			stateFile('both'),
			'--runner',
			runner,
			'--tasks',
			'tasks.json',
		]);
		expect(result.code).toBe(2);
		expect(result.err).toContain('--runner and --tasks both name the task code: pass one');
	});

	test('refuses --each without --key, and --key without --each', async () => {
		const each = await runEntry(durableCli, [list, stateFile('each'), '--each', item]);
		expect(each.code).toBe(2);
		expect(each.err).toContain('--each and --key go together');
		const key = await runEntry(durableCli, [list, stateFile('key'), '--key', 'domain']);
		expect(key.code).toBe(2);
		expect(key.err).toContain('--each and --key go together');
	});

	test('refuses --seen and --until-passed without --each', async () => {
		const result = await runEntry(durableCli, [
			workflow,
			stateFile('seen'),
			'--runner',
			runner,
			'--seen',
			'seen.json',
		]);
		expect(result.code).toBe(2);
		expect(result.err).toContain('--seen and --until-passed apply to --each only');
	});

	test('refuses a count for --until-passed that is not a whole number', async () => {
		const result = await runEntry(durableCli, [
			list,
			stateFile('count'),
			'--each',
			item,
			'--key',
			'domain',
			'--until-passed',
			'many',
		]);
		expect(result.code).toBe(2);
		expect(result.err).toContain('--until-passed needs a whole number, got many');
	});

	test('refuses a graph with tasks when it has no runner or commands', async () => {
		const result = await runEntry(durableCli, [workflow, stateFile('no-runner')]);
		expect(result.code).toBe(3);
		expect(result.err).toContain('but no runner or commands');
	});

	test('a flag without its value is named', async () => {
		const result = await runEntry(durableCli, [workflow, stateFile('value'), '--input']);
		expect(result.code).toBe(2);
		expect(result.err).toContain('--input needs a value');
	});

	test('runs a graph with a runner and prints its result as JSON', async () => {
		const result = await runEntry(durableCli, [workflow, stateFile('run'), '--runner', runner]);
		expect(result.code).toBe(0);
		expect(result.out).toContain('"status":"ended"');
	});

	test('fans one run out per item, and a second run starts none', async () => {
		const state = stateFile('fan');
		const args = [list, state, '--runner', fanOutRunner(), '--each', item, '--key', 'domain'];
		const first = await runEntry(durableCli, args);
		expect(first.code).toBe(0);
		expect(first.out).toContain('"status":"ended","items":2,"started":2,"passed":2');
		const second = await runEntry(durableCli, args);
		expect(second.out).toContain('"items":2,"started":0');
	});
});

describe('simulate', () => {
	test('simulates the graph with each task passing and exits 0 when the run ends', async () => {
		const result = await runEntry(simulate, [workflow]);
		expect(result.code).toBe(0);
		expect(result.out.length).toBeGreaterThan(0);
	});

	test('a stub that names no task of the graph exits 2', async () => {
		const result = await runEntry(simulate, [workflow, 'nope=pass']);
		expect(result.code).toBe(2);
		expect(result.err).toContain('no task "nope" in the graph');
	});
});

describe('debug cli', () => {
	test('prints the usage and exits 2 without a graph', async () => {
		const result = await runEntry(debugCli, []);
		expect(result.code).toBe(2);
		expect(result.err).toContain('usage: bun cli/debug.ts <dir>/<graph>.ts');
	});

	test('runs the given commands on the graph and prints their output', async () => {
		const result = await runEntry(debugCli, [walk, 'show']);
		expect(result.code).toBe(0);
		expect(result.out).toContain('node');
	});
});
