// Command-backed tasks. Each task runs as a command, instead of a function in a runner module.
//
// The command gets one JSON object on stdin:
//   { "task", "key", "attempt", "input", "failure" }
// `input` is the run's input (null when there is none). `failure` is the run's last failed stage,
// with its node, task, key, outcome and detail, or null. `outputs` is the directory where earlier
// stages' outputs are kept, as `<node>.json`, or null.
//
// The command reports its result on stdout, as one JSON object:
//   { "outcome": "pass" | "fail" | "none", "detail"?: { "message", "code"?, "data"? } }
// Empty stdout with exit status 0 means `pass`. A nonzero exit status is a thrown attempt, so a
// retry node tries again. Stderr goes to this process's stderr, where the run log also writes.
import { spawn } from 'node:child_process';
import type { Runner, RunnerReport, TaskContext } from './durable';

/** The tasks the host can run as commands: each task name, and the argv that runs it. */
export interface CommandTable {
	/** The manifest the table came from, for messages. */
	readonly source?: string;
	readonly cwd: string;
	readonly commands: Readonly<Record<string, readonly string[]>>;
}

/** A runner that runs each task's command, from `table`. */
export function commandRunner(table: CommandTable): Runner {
	return (task: string, context: TaskContext) => {
		const argv = Object.hasOwn(table.commands, task)
			? table.commands[task]
			: undefined;
		if (argv === undefined || argv.length === 0) {
			return Promise.reject(new Error(`no command for task ${task}`));
		}
		const request = {
			task,
			key: context.key,
			attempt: context.attempt,
			input: context.input ?? null,
			failure: context.failure ?? null,
			outputs: context.outputs.directory ?? null,
		};
		return runCommand(argv, table.cwd, request, context.signal);
	};
}

function runCommand(
	argv: readonly string[],
	cwd: string,
	request: unknown,
	signal: AbortSignal
): Promise<RunnerReport> {
	const [program = '', ...args] = argv;
	return new Promise((resolve, reject) => {
		const child = spawn(program, args, {
			cwd,
			stdio: ['pipe', 'pipe', 'inherit'],
		});
		let stdout = '';
		child.stdout.setEncoding('utf8');
		child.stdout.on('data', (chunk: string) => {
			stdout += chunk;
		});
		// A command that exits before reading stdin is not an error of its own; its exit status is.
		child.stdin.on('error', () => {});
		child.stdin.end(JSON.stringify(request));

		const stop = () => child.kill();
		signal.addEventListener('abort', stop, { once: true });
		child.on('error', (error) => {
			signal.removeEventListener('abort', stop);
			reject(error);
		});
		child.on('close', (code) => {
			signal.removeEventListener('abort', stop);
			if (signal.aborted) {
				reject(new Error(`${program} was stopped after its timeout`));
				return;
			}
			if (code !== 0) {
				reject(new Error(`${program} exited with status ${code ?? 'none'}`));
				return;
			}
			try {
				resolve(reportOf(stdout));
			} catch (error) {
				// Thrown inside an event listener, so it must reject here, or the promise never settles.
				reject(error);
			}
		});
	});
}

/** The report a command printed. Empty output is a pass. Anything else must be one JSON object. */
function reportOf(stdout: string): RunnerReport {
	const text = stdout.trim();
	if (text === '') return { outcome: 'pass' };
	const value: unknown = JSON.parse(text);
	return value as RunnerReport;
}
