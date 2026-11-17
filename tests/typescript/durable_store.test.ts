// The file store's results file: a rewrite drops records of earlier stages, and a periodic compaction
// does the same during a run. Run by `tests/typescript_durable_units.rs`.
import { describe, expect, test } from 'bun:test';
import { appendFileSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
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
