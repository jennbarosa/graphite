// The file-backed state store: the snapshot file, the results file, the outputs directory, and the
// lock. Each write is durable before it returns.

import {
	closeSync,
	existsSync,
	fstatSync,
	fsyncSync,
	ftruncateSync,
	mkdirSync,
	openSync,
	readFileSync,
	readSync,
	renameSync,
	unlinkSync,
	writeFileSync,
	writeSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import type { Detail, StateStore, StoredOutcome, StoredResult } from './contract';
import { isDetail, isRecord, isStoredOutcome } from './report';

const READ_CHUNK_BYTES = 64 * 1024;

/**
 * The state store backed by local files. `<file>` holds the snapshot, rewritten atomically at
 * each step. `<file>.results` holds one JSON line per recorded result, appended and fsynced as it
 * is recorded. `<file>.lock` lets one process at a time use the state.
 */
export function fileStore(stateFile: string): StateStore {
	const resultsFile = `${stateFile}.results`;
	const outputDir = `${stateFile}.outputs`;
	return {
		outputDirectory: outputDir,
		saveOutput: (node, value) => {
			mkdirSync(outputDir, { recursive: true });
			writeAtomic(join(outputDir, `${node}.json`), JSON.stringify(value));
		},
		loadOutput: (node) => {
			const file = join(outputDir, `${node}.json`);
			return existsSync(file)
				? JSON.parse(readFileSync(file, 'utf8'))
				: undefined;
		},
		loadSnapshot: () =>
			existsSync(stateFile) ? readSnapshot(stateFile) : undefined,
		loadRecords: (since) => readRecords(resultsFile, since),
		saveSnapshot: (snapshot) =>
			writeAtomic(stateFile, JSON.stringify({ snapshot })),
		recordResult: (key, sequence, result) =>
			appendLine(
				resultsFile,
				JSON.stringify({
					key,
					sequence,
					outcome: result?.outcome ?? null,
					detail: result?.detail,
				})
			),
		withLock: (body) => withLock(stateFile, body),
	};
}

function readSnapshot(file: string): unknown {
	const parsed: unknown = JSON.parse(readFileSync(file, 'utf8'));
	if (!isRecord(parsed) || !('snapshot' in parsed)) {
		throw new Error(`state file ${file} is malformed`);
	}
	return parsed.snapshot;
}

interface ResultRecord {
	readonly key: string;
	readonly sequence: number;
	readonly outcome: StoredOutcome | null;
	readonly detail?: Detail | undefined;
}

function parseRecord(line: string): ResultRecord | undefined {
	try {
		const value: unknown = JSON.parse(line);
		if (
			!isRecord(value) ||
			typeof value.key !== 'string' ||
			typeof value.sequence !== 'number'
		) {
			return undefined;
		}
		const outcome = value.outcome;
		if (outcome !== null && !isStoredOutcome(outcome)) {
			return undefined;
		}
		if (value.detail !== undefined && !isDetail(value.detail)) {
			return undefined;
		}
		return {
			key: value.key,
			sequence: value.sequence,
			outcome,
			detail: value.detail,
		};
	} catch {
		return undefined;
	}
}

/**
 * Reads the records at `since` or later, in order, keeping the last result for each key. The
 * file is read in chunks, so only those records are held in memory. A last line cut short by a
 * crash is dropped, and the file is cut back to the end of the last complete line, so later
 * appends start on a clean line. A bad complete line means the file is damaged.
 */
function readRecords(file: string, since: number): Map<string, StoredResult> {
	const latest = new Map<string, StoredResult | null>();
	if (!existsSync(file)) return new Map();
	const fd = openSync(file, 'r+');
	try {
		const size = fstatSync(fd).size;
		const completeEnd = forEachLine(fd, (line) => {
			if (line === '') return;
			const record = parseRecord(line);
			if (record === undefined) {
				throw new Error(`results file ${file} is malformed`);
			}
			if (record.sequence >= since) {
				latest.set(
					record.key,
					record.outcome === null
						? null
						: record.detail === undefined
							? { outcome: record.outcome }
							: { outcome: record.outcome, detail: record.detail }
				);
			}
		});
		if (completeEnd < size) {
			ftruncateSync(fd, completeEnd);
			fsyncSync(fd);
		}
	} finally {
		closeSync(fd);
	}
	const recovered = new Map<string, StoredResult>();
	for (const [key, result] of latest) {
		if (result !== null) recovered.set(key, result);
	}
	return recovered;
}

/**
 * Calls `visit` with each complete line, in order, reading the file in chunks. Returns the byte
 * offset just past the last complete line. Text after it is an unfinished line.
 */
function forEachLine(fd: number, visit: (line: string) => void): number {
	const chunk = Buffer.alloc(READ_CHUNK_BYTES);
	let carry = Buffer.alloc(0);
	let offset = 0;
	let completeEnd = 0;
	for (;;) {
		const bytes = readSync(fd, chunk, 0, READ_CHUNK_BYTES, offset);
		if (bytes === 0) break;
		const data = Buffer.concat([carry, chunk.subarray(0, bytes)]);
		const dataStart = offset - carry.length;
		offset += bytes;
		let start = 0;
		let newline = data.indexOf(0x0a);
		while (newline !== -1) {
			visit(data.subarray(start, newline).toString('utf8'));
			start = newline + 1;
			completeEnd = dataStart + start;
			newline = data.indexOf(0x0a, start);
		}
		carry = Buffer.from(data.subarray(start));
	}
	return completeEnd;
}

/** Appends one line and makes it durable before returning. */
export function appendLine(file: string, line: string): void {
	const isNew = !existsSync(file);
	const fd = openSync(file, 'a');
	try {
		writeFileSync(fd, `${line}\n`);
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
	if (isNew) fsyncDir(dirname(file));
}

/** Temp file, fsync, rename, then fsync the directory so the rename itself is durable. */
function writeAtomic(file: string, text: string): void {
	const temp = `${file}.${process.pid}.tmp`;
	const fd = openSync(temp, 'w');
	try {
		writeFileSync(fd, text);
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
	renameSync(temp, file);
	fsyncDir(dirname(file));
}

function fsyncDir(dir: string): void {
	const fd = openSync(dir, 'r');
	try {
		fsyncSync(fd);
	} finally {
		closeSync(fd);
	}
}

async function withLock<T>(
	stateFile: string,
	body: () => T | Promise<T>
): Promise<T> {
	const release = acquire(stateFile);
	try {
		return await body();
	} finally {
		release();
	}
}

/**
 * Creates `<state>.lock` holding this process's pid. A lock whose pid is no longer running is
 * taken over. shortcut: the takeover is not atomic, so two operators recovering at the same
 * instant could both win; upgrade to flock if that happens.
 */
function acquire(stateFile: string): () => void {
	const lock = `${stateFile}.lock`;
	for (let tries = 0; tries < 2; tries++) {
		try {
			const fd = openSync(lock, 'wx');
			writeSync(fd, String(process.pid));
			closeSync(fd);
			return () => unlinkSync(lock);
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== 'EEXIST')
				throw error;
			const owner = Number(readFileSync(lock, 'utf8'));
			if (alive(owner)) {
				throw new Error(
					`state ${stateFile} is locked by process ${owner}`
				);
			}
			unlinkSync(lock);
		}
	}
	throw new Error(`could not lock ${stateFile}`);
}

function alive(pid: number): boolean {
	if (!Number.isInteger(pid) || pid <= 0) return false;
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		return (error as NodeJS.ErrnoException).code === 'EPERM';
	}
}
