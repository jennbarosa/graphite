// The file store's results file: a rewrite drops records of earlier stages, and a periodic compaction
// does the same during a run. Run by `tests/typescript_durable_units.rs`.
import { describe, expect, test } from 'bun:test';
import { appendFileSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { retryFailedTask } from '../../src/frontend/typescript/durable/engine';
import { fileStore } from '../../src/frontend/typescript/durable/store';

/** A fresh state file path, and its results file. */
function paths(): { state: string; results: string } {
	const state = join(mkdtempSync(join(tmpdir(), 'graphite-store-')), 'state.json');
	return { state, results: `${state}.results` };
}

/** The keys of the records in the results file, in order. */
function keysIn(file: string): string[] {
	return readFileSync(file, 'utf8')
		.split('\n')
		.filter((line) => line !== '')
		.map((line) => (JSON.parse(line) as { key: string }).key);
}

describe('results file', () => {
	test('loading records rewrites the file without records of earlier stages', () => {
		const { state, results } = paths();
		const store = fileStore(state);
		store.recordResult('0:a', 0, { outcome: 'pass' });
		store.recordResult('2:b', 2, { outcome: 'pass' });
		store.recordResult('3:c', 3, { outcome: 'fail' });

		expect([...store.loadRecords(2).keys()]).toEqual(['2:b', '3:c']);
		expect(keysIn(results)).toEqual(['2:b', '3:c']);
	});

	test('compaction runs once every 1000 steps and drops earlier records', () => {
		const { state, results } = paths();
		const store = fileStore(state);
		store.recordResult('0:a', 0, { outcome: 'pass' });
		store.recordResult('5:b', 5, { outcome: 'pass' });

		for (let step = 0; step < 999; step++) store.compactResults?.(5);
		expect(keysIn(results)).toEqual(['0:a', '5:b']);

		store.compactResults?.(5);
		expect(keysIn(results)).toEqual(['5:b']);
	});

	test('a rewrite drops a last line that a crash cut short', () => {
		const { state, results } = paths();
		const store = fileStore(state);
		store.recordResult('0:a', 0, { outcome: 'pass' });
		store.recordResult('1:b', 1, { outcome: 'pass' });
		appendFileSync(results, '{"key":"1:c","seq');

		expect([...store.loadRecords(1).keys()]).toEqual(['1:b']);
		expect(readFileSync(results, 'utf8').endsWith('\n')).toBe(true);
		expect(keysIn(results)).toEqual(['1:b']);
	});
});

describe('state lock', () => {
	test('a lock with no owner in it is refused, not taken over as a dead run', async () => {
		const { state } = paths();
		// The old lock was created empty, then the pid was written. A reader in that gap saw no owner.
		writeFileSync(`${state}.lock`, '');
		const store = fileStore(state);
		await expect(store.withLock(() => 1)).rejects.toThrow(/locked/);
		expect(readFileSync(`${state}.lock`, 'utf8')).toBe('');
	});

	test('while held, the lock names this process; it is removed afterwards', async () => {
		const { state } = paths();
		const store = fileStore(state);
		await store.withLock(() => {
			expect(readFileSync(`${state}.lock`, 'utf8')).toBe(String(process.pid));
		});
		expect(existsSync(`${state}.lock`)).toBe(false);
	});
});

describe('retryFailedTask', () => {
	test('refuses a stored error from a stage the saved run has already passed', async () => {
		const { state } = paths();
		const store = fileStore(state);
		store.recordResult('0:a', 0, { outcome: 'error', detail: { message: 'boom' } });
		// The run has moved past stage 0: its saved position is transition 2.
		store.saveSnapshot({ run: { transitions: 2 }, lastFailure: null, inputDigest: null });

		await expect(retryFailedTask(store, '0:a')).rejects.toThrow(/the run is past this stage/);
		expect(store.loadRecords(0).get('0:a')?.outcome).toBe('error');
	});

	test('clears a stored error from the stage the run is on', async () => {
		const { state } = paths();
		const store = fileStore(state);
		store.recordResult('2:a', 2, { outcome: 'error', detail: { message: 'boom' } });
		store.saveSnapshot({ run: { transitions: 2 }, lastFailure: null, inputDigest: null });

		await retryFailedTask(store, '2:a');
		expect(store.loadRecords(0).has('2:a')).toBe(false);
	});
});
