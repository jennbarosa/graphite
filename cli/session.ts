// A debugging session over one generated graph. Commands go in as text lines and come out as
// text; the session owns the run, the edges taken so far, and the condition values.
import { dirname, join, resolve } from 'node:path';
import {
	HELP,
	USAGE_CHOOSE,
	USAGE_SET,
	formatArm,
	formatState,
	formatStep,
} from './format';
import type { GraphLike, RunLike } from './types';

export interface Session {
	/** Runs one command line. Returns the text to print, or `null` to quit. */
	execute(line: string): string | null;
}

/** Edges one `step` may record. A longer settle reports `output_full`. */
const TRACE_CAPACITY = 1024;

type Handler = (args: string[]) => string | null;

class DebugSession implements Session {
	private run: RunLike;
	private readonly edges: number[] = [];
	private readonly trace = new Uint32Array(TRACE_CAPACITY);
	private readonly handlers: Readonly<Record<string, Handler>>;

	constructor(
		private readonly graph: GraphLike,
		private readonly runtime: { start(graph: GraphLike): RunLike }
	) {
		this.run = runtime.start(graph);
		this.handlers = {
			'': () => '',
			help: () => HELP,
			quit: () => null,
			exit: () => null,
			show: () => this.show(),
			set: (args) => this.set(args),
			finish: () => this.finish(),
			step: () => this.step(),
			choose: (args) => this.choose(args),
			history: () => this.history(),
			reset: () => this.reset(),
		};
	}

	execute(line: string): string | null {
		const [command = '', ...args] = line.trim().split(/\s+/);
		try {
			const handler = Object.hasOwn(this.handlers, command)
				? this.handlers[command]
				: undefined;
			return handler === undefined
				? `unknown command "${command}"; try help`
				: handler(args);
		} catch (error) {
			return `error: ${error instanceof Error ? error.message : String(error)}`;
		}
	}

	private show(): string {
		const truths = this.graph
			.conditionNames()
			.filter((name) => this.run.holds(name));
		return formatState(this.run, truths);
	}

	private set(args: string[]): string {
		const [name = '', value = ''] = args;
		if (args.length !== 2 || (value !== 'on' && value !== 'off')) {
			return USAGE_SET;
		}
		this.run.set(name, value === 'on');
		return `${name} ${value}`;
	}

	private finish(): string {
		this.run.finish();
		return this.show();
	}

	private step(): string {
		const result = this.run.step(this.trace);
		const taken = Array.from(this.trace.subarray(0, result.taken));
		this.edges.push(...taken);
		return formatStep(this.show(), taken);
	}

	private choose(args: string[]): string {
		const [name] = args;
		if (name === undefined || args.length !== 1) {
			return USAGE_CHOOSE;
		}
		const arm = this.run.choose(name);
		return arm === undefined
			? `${name}: no arm selected`
			: formatArm(name, arm);
	}

	private history(): string {
		return this.edges.length === 0
			? 'no edges taken'
			: this.edges.join(' ');
	}

	private reset(): string {
		this.run = this.runtime.start(this.graph);
		this.edges.length = 0;
		return `reset: ${this.show()}`;
	}
}

/** Loads the graph in `modulePath` and its runtime, which sits beside it, and starts a session. */
export async function openSession(modulePath: string): Promise<Session> {
	const file = resolve(modulePath);
	const runtime = await import(join(dirname(file), 'graphite.ts'));
	const module = await import(file);
	return new DebugSession(module.graph as GraphLike, runtime);
}
