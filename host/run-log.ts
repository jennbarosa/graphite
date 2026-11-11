// A readable log of one run, built on the host's `onEvent`. Pass `runLog(console.error)` as
// `onEvent` to turn logging on. Each line names a stage, a task, or an attempt by the graph's own
// names and the host's keys, and the last line summarizes how often each stage and task ran.
import type { HostEvent } from './durable';

export type LogSink = (line: string) => void;

export interface RunLogOptions {
	/** Prefix each line with the time it was written. Default true. */
	readonly timestamps?: boolean;
	/** The time text for a line. Default is the current time in ISO 8601, in UTC. */
	readonly clock?: () => string;
}

/** Returns an `onEvent` handler that writes one line per event to `sink`. */
export function runLog(
	sink: LogSink,
	options: RunLogOptions = {}
): (event: HostEvent) => void {
	const visits = new Map<string, number>();
	const settled = new Map<string, number>();
	const clock = options.clock ?? (() => new Date().toISOString());
	const stamped = options.timestamps ?? true;
	const write = (line: string) =>
		sink(stamped ? `${clock()} ${line}` : line);

	const count = (map: Map<string, number>, name: string) =>
		map.set(name, (map.get(name) ?? 0) + 1);
	const summary = (map: Map<string, number>) =>
		[...map]
			.map(([name, n]) => (n > 1 ? `${name} x${n}` : name))
			.join(', ') || 'none';

	return (event) => {
		switch (event.type) {
			case 'run-started':
				visits.clear();
				settled.clear();
				write(
					`[run] started ${event.resumed ? 'resumed' : 'fresh'}, ${event.storedResults} stored result(s)`
				);
				return;
			case 'stage-entered':
				count(visits, event.node);
				write(`[stage] ${event.node} (key ${event.key})`);
				return;
			case 'attempt-failed':
				write(
					`[attempt] ${event.task} attempt ${event.attempt} failed (key ${event.key}): ${event.error}`
				);
				return;
			case 'task-settled':
				count(settled, event.task);
				write(
					`[task] ${event.task} ${event.outcome} (${event.source}, key ${event.key})`
				);
				return;
			case 'step-taken':
				write(
					`[step] ${event.taken} edge(s), now at ${event.node}`
				);
				return;
			case 'run-stopped': {
				const r = event.result;
				const detail =
					r.status === 'error'
						? `${r.error} (key ${r.key})`
						: r.status === 'stalled'
							? `waiting at ${r.node}`
							: '';
				write(
					`[run] stopped ${r.status}${detail ? `: ${detail}` : ''}`
				);
				write(`[run] stages: ${summary(visits)}`);
				write(`[run] tasks: ${summary(settled)}`);
				return;
			}
		}
	};
}
