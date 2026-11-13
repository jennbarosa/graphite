// Graphite runtime. Emitted verbatim next to each generated graph module.
// Node and choice IDs are global to a graph. Each chunk holds a contiguous ID range,
// and its records use spans local to that chunk. Targets and condition indices are global.

export type Span = readonly [number, number];

export interface NodeRecord {
	readonly name: number;
	readonly looping: boolean;
	readonly end: boolean;
	readonly flags: Span;
	readonly annotations: Span;
	/** Span into this chunk's `edges`, in source order. */
	readonly outgoing: Span;
	/** Local index into this chunk's `edges`, or null. */
	readonly sequence: number | null;
	readonly offset: number;
}

export interface EdgeRecord {
	readonly id: number;
	readonly target: number;
	readonly condition: number | null;
	readonly annotations: Span;
}

export interface AnnotationRecord {
	readonly key: number;
	readonly value: number | null;
}

/**
 * A chunk's records, built from its columns on each access. Records are plain objects, so an
 * access allocates; the columns themselves stay compact.
 */
export class Table<T> {
	constructor(
		readonly length: number,
		private readonly build: (index: number) => T
	) {}

	/** The record at `index`. Throws for an index outside the table. */
	get(index: number): T {
		if (!Number.isInteger(index) || index < 0 || index >= this.length) {
			throw new RangeError(`index ${index} is out of range`);
		}
		return this.build(index);
	}

	forEach(visit: (record: T, index: number) => void): void {
		for (let index = 0; index < this.length; index++) {
			visit(this.build(index), index);
		}
	}

	*[Symbol.iterator](): Iterator<T> {
		for (let index = 0; index < this.length; index++) {
			yield this.build(index);
		}
	}
}

export interface NodeChunk {
	readonly nodes: Table<NodeRecord>;
	readonly edges: Table<EdgeRecord>;
	readonly annotations: Table<AnnotationRecord>;
	readonly flags: readonly number[];
}

/** How many records of each kind a node chunk holds. */
export interface NodeChunkCounts {
	readonly nodes: number;
	readonly edges: number;
	readonly annotations: number;
	readonly flags: number;
}

/** The value stored in a column for `null`. No index or ID in a chunk reaches it. */
const NONE = 0xffffffff;

/**
 * Decodes a node chunk from its base64 text. The text is little-endian 32-bit words, one column
 * after another, in the order the emitter writes them. A column of spans holds start and end in
 * pairs. Booleans are bits of the node's `bits` column: 1 for looping, 2 for end.
 */
export function decodeNodeChunk(
	counts: NodeChunkCounts,
	encoded: string
): NodeChunk {
	const binary = atob(encoded);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	const words = new Uint32Array(bytes.buffer);
	let next = 0;
	const column = (length: number): Uint32Array => {
		const values = words.subarray(next, next + length);
		if (values.length !== length)
			throw new RangeError('chunk data is truncated');
		next += length;
		return values;
	};
	const cell = (values: Uint32Array, index: number): number => {
		const value = values[index];
		if (value === undefined)
			throw new RangeError(`index ${index} is out of range`);
		return value;
	};
	const pair = (values: Uint32Array, index: number): Span => [
		cell(values, 2 * index),
		cell(values, 2 * index + 1),
	];
	const orNull = (value: number): number | null =>
		value === NONE ? null : value;
	const { nodes: n, edges: e, annotations: a, flags: f } = counts;
	const name = column(n);
	const offset = column(n);
	const sequence = column(n);
	const bits = column(n);
	const nodeFlags = column(2 * n);
	const nodeAnnotations = column(2 * n);
	const outgoing = column(2 * n);
	const edgeId = column(e);
	const edgeTarget = column(e);
	const edgeCondition = column(e);
	const edgeAnnotations = column(2 * e);
	const key = column(a);
	const value = column(a);
	const flags = Array.from(column(f));
	if (next !== words.length)
		throw new RangeError('chunk data has extra words');

	return {
		nodes: new Table(n, (index) => {
			const flag = cell(bits, index);
			return {
				name: cell(name, index),
				looping: (flag & 1) !== 0,
				end: (flag & 2) !== 0,
				flags: pair(nodeFlags, index),
				annotations: pair(nodeAnnotations, index),
				outgoing: pair(outgoing, index),
				sequence: orNull(cell(sequence, index)),
				offset: cell(offset, index),
			};
		}),
		edges: new Table(e, (index) => ({
			id: cell(edgeId, index),
			target: cell(edgeTarget, index),
			condition: orNull(cell(edgeCondition, index)),
			annotations: pair(edgeAnnotations, index),
		})),
		annotations: new Table(a, (index) => ({
			key: cell(key, index),
			value: orNull(cell(value, index)),
		})),
		flags,
	};
}

export interface ChoiceRecord {
	readonly name: number;
	/** Span into this chunk's `arms`. */
	readonly arms: Span;
}

export interface ArmRecord {
	readonly condition: number | null;
	readonly tag: number;
	readonly annotations: Span;
}

export interface ChoiceChunk {
	readonly choices: readonly ChoiceRecord[];
	readonly arms: readonly ArmRecord[];
	readonly annotations: readonly AnnotationRecord[];
}

export interface GraphData {
	readonly name: string;
	readonly strings: readonly string[];
	/** The annotations on the `graph` line, as string IDs: `[key, value]`, with a null value for `@key`. */
	readonly annotations: readonly (readonly [number, number | null])[];
	readonly conditions: readonly number[];
	readonly flags: readonly number[];
	/** Nodes per chunk is `1 << nodeShift`. */
	readonly nodeShift: number;
	readonly nodeCount: number;
	readonly nodeChunks: readonly NodeChunk[];
	/** Choices per chunk is `1 << choiceShift`. */
	readonly choiceShift: number;
	readonly choiceCount: number;
	readonly choiceChunks: readonly ChoiceChunk[];
}

export interface Annotation {
	readonly key: string;
	readonly value: string | null;
}

export interface Arm {
	readonly condition: string | null;
	readonly tag: string;
	readonly annotations: readonly Annotation[];
}

/** One transition, as `Graph.edge` describes it. */
export interface EdgeInfo {
	readonly id: number;
	readonly source: string;
	readonly target: string;
	/** The condition that gates the edge, or null for a sequence edge. */
	readonly condition: string | null;
	readonly annotations: readonly Annotation[];
}

/** The name vocabulary a generated module supplies as its type parameter. */
export interface Vocabulary {
	readonly nodes: string;
	readonly conditions: string;
	readonly flags: string;
	readonly annotations: Annotation;
	readonly choices: { readonly [choice: string]: Arm };
}

export type Status = 'running' | 'ended' | 'error';
export type ErrorCode = 'cycle' | 'output_full' | 'limit';
const RESTORABLE_FAILURES: readonly ErrorCode[] = [
	'cycle',
	'output_full',
	'limit',
];

/**
 * A run's state as plain data, to store and restore later. `graph` fingerprints the graph
 * the run belongs to, so a snapshot cannot be restored into a different one.
 */
export interface RunSnapshot {
	readonly version: 1;
	readonly graph: string;
	readonly node: number;
	readonly phase: Exclude<Status, 'error'>;
	readonly failure: ErrorCode | null;
	readonly finished: boolean;
	readonly transitions: number;
	readonly bits: readonly number[];
}

export interface RunOptions {
	/**
	 * Most transitions this run may take in total, counted from `start`. A transition past the
	 * limit makes the run error with `limit` before it is taken. Unset means no limit.
	 */
	readonly maxTransitions?: number;
}

interface StepBase<V extends Vocabulary> {
	readonly node: V['nodes'];
	/** Transitions taken by this run since `start`. */
	readonly transitions: number;
	/** Transitions applied by this call. */
	readonly taken: number;
}

/** `error` is non-null exactly when `status` is `'error'`. */
export type StepResult<V extends Vocabulary> = StepBase<V> &
	(
		| { readonly status: 'error'; readonly error: ErrorCode }
		| {
				readonly status: Exclude<Status, 'error'>;
				readonly error: null;
		  }
	);

function at<T>(items: readonly T[], index: number): T {
	const value = items[index];
	if (value === undefined) {
		throw new RangeError(`index ${index} is out of range`);
	}
	return value;
}

/** 32-bit FNV-1a over UTF-16 code units, from the given offset basis. */
function fnv1a(text: string, basis: number): string {
	let hash = basis >>> 0;
	for (let index = 0; index < text.length; index++) {
		hash ^= text.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193) >>> 0;
	}
	return hash.toString(16).padStart(8, '0');
}

/** Immutable compiled graph with name lookups resolved once. */
export class Graph<V extends Vocabulary> {
	readonly name: string;
	readonly data: GraphData;
	private readonly nodeMask: number;
	private readonly choiceMask: number;
	private readonly conditionIds = new Map<string, number>();
	private readonly choiceIds = new Map<string, number>();
	private readonly nodeIds = new Map<string, number>();
	private fingerprintValue: string | undefined;
	/** Built on the first `edge` call, so graphs that never ask pay nothing for it. */
	private edgeTable: Map<number, EdgeInfo> | undefined;

	constructor(data: GraphData) {
		this.name = data.name;
		this.data = data;
		this.nodeMask = (1 << data.nodeShift) - 1;
		this.choiceMask = (1 << data.choiceShift) - 1;
		data.conditions.forEach((name, id) =>
			this.conditionIds.set(this.text(name), id)
		);
		data.nodeChunks.forEach((chunk, index) =>
			chunk.nodes.forEach((node, local) =>
				this.nodeIds.set(
					this.text(node.name),
					(index << data.nodeShift) + local
				)
			)
		);
		data.choiceChunks.forEach((chunk, index) =>
			chunk.choices.forEach((choice, local) =>
				this.choiceIds.set(
					this.text(choice.name),
					(index << data.choiceShift) + local
				)
			)
		);
	}

	/** Number of conditions in the graph; sizes a run's bitset. */
	get conditionCount(): number {
		return this.data.conditions.length;
	}

	/** Resolves a condition name once; keep the ID for hot paths. */
	conditionId(name: V['conditions']): number {
		const id = this.conditionIds.get(name);
		if (id === undefined) {
			throw new RangeError(
				`unknown condition "${name}" in graph "${this.name}"`
			);
		}
		return id;
	}

	choiceId(name: keyof V['choices'] & string): number {
		const id = this.choiceIds.get(name);
		if (id === undefined) {
			throw new RangeError(
				`unknown choice "${name}" in graph "${this.name}"`
			);
		}
		return id;
	}

	/** Every condition name in the graph, in declaration order. */
	conditionNames(): V['conditions'][] {
		return this.data.conditions.map(
			(id) => this.text(id) as V['conditions']
		);
	}

	/**
	 * Identifies the graph's structure: names, edges, conditions and annotations. Changing any
	 * of them changes the fingerprint. Not a cryptographic hash; it catches mismatches, not tampering.
	 */
	fingerprint(): string {
		this.fingerprintValue ??= this.computeFingerprint();
		return this.fingerprintValue;
	}

	private computeFingerprint(): string {
		const parts: string[] = [this.name, String(this.data.nodeCount)];
		for (const { key, value } of this.annotations())
			parts.push(key, value ?? '');
		for (const id of this.data.conditions) parts.push(this.text(id));
		parts.push('|');
		for (let id = 0; id < this.data.nodeCount; id++) {
			const node = this.nodeRecord(id);
			const chunk = this.nodeChunk(id);
			parts.push(
				this.text(node.name),
				node.end ? 'end' : '-',
				node.looping ? 'loop' : '-'
			);
			for (const annotation of this.annotationsIn(
				chunk.annotations,
				node.annotations
			)) {
				parts.push(annotation.key, annotation.value ?? '');
			}
			const [start, end] = node.outgoing;
			for (let index = start; index < end; index++) {
				const edge = chunk.edges.get(index);
				parts.push(
					String(edge.id),
					String(edge.target),
					edge.condition === null
						? '-'
						: String(edge.condition)
				);
				for (const annotation of this.annotationsIn(
					chunk.annotations,
					edge.annotations
				)) {
					parts.push(
						annotation.key,
						annotation.value ?? ''
					);
				}
			}
			parts.push('|');
		}
		const text = parts.join('\u0000');
		return fnv1a(text, 0x811c9dc5) + fnv1a(text, 0x01000193);
	}

	/** Annotations on the `graph` line, in source order. */
	annotations(): readonly Annotation[] {
		return this.data.annotations.map(([key, value]) => ({
			key: this.text(key),
			value: value === null ? null : this.text(value),
		}));
	}

	/** The ID of node `name`. Throws for a name the graph does not have. */
	nodeId(name: V['nodes']): number {
		const id = this.nodeIds.get(name);
		if (id === undefined) {
			throw new RangeError(
				`unknown node "${name}" in graph "${this.name}"`
			);
		}
		return id;
	}

	/** Annotations on node `name`, in source order. */
	nodeAnnotations(name: V['nodes']): readonly Annotation[] {
		const id = this.nodeId(name);
		return this.annotationsIn(
			this.nodeChunk(id).annotations,
			this.nodeRecord(id).annotations
		);
	}

	/** Whether node `name` was declared `loop`. Throws for an unknown name. */
	nodeLooping(name: V['nodes']): boolean {
		return this.nodeRecord(this.nodeId(name)).looping;
	}

	/** Whether node `name` was declared `end`. Throws for an unknown name. */
	nodeEnd(name: V['nodes']): boolean {
		return this.nodeRecord(this.nodeId(name)).end;
	}

	/** Domain flags on node `name`, in source order. Core flags are not included. */
	nodeFlags(name: V['nodes']): readonly string[] {
		const id = this.nodeId(name);
		const [start, end] = this.nodeRecord(id).flags;
		return this.nodeChunk(id)
			.flags.slice(start, end)
			.map((flag) => this.text(flag));
	}

	/**
	 * The transition with edge ID `id`, as `Run.step` reports it. Throws for an ID the graph
	 * does not have.
	 */
	edge(id: number): EdgeInfo {
		this.edgeTable ??= this.buildEdgeTable();
		const info = this.edgeTable.get(id);
		if (info === undefined) {
			throw new RangeError(
				`unknown edge ${id} in graph "${this.name}"`
			);
		}
		return info;
	}

	private buildEdgeTable(): Map<number, EdgeInfo> {
		const table = new Map<number, EdgeInfo>();
		for (const chunk of this.data.nodeChunks) {
			for (const node of chunk.nodes) {
				const [start, end] = node.outgoing;
				for (let index = start; index < end; index++) {
					const edge = chunk.edges.get(index);
					table.set(edge.id, {
						id: edge.id,
						source: this.text(node.name),
						target: this.nodeName(edge.target),
						condition:
							edge.condition === null
								? null
								: this.conditionName(
										edge.condition
									),
						annotations: this.annotationsIn(
							chunk.annotations,
							edge.annotations
						),
					});
				}
			}
		}
		return table;
	}

	/** The chunk holding node `id`. */
	nodeChunk(id: number): NodeChunk {
		return at(this.data.nodeChunks, id >> this.data.nodeShift);
	}

	nodeIndex(id: number): number {
		return id & this.nodeMask;
	}

	nodeRecord(id: number): NodeRecord {
		return this.nodeChunk(id).nodes.get(this.nodeIndex(id));
	}

	nodeName(id: number): V['nodes'] {
		return this.text(this.nodeRecord(id).name) as V['nodes'];
	}

	/** The chunk holding choice `id`. */
	choiceChunk(id: number): ChoiceChunk {
		return at(this.data.choiceChunks, id >> this.data.choiceShift);
	}

	choiceIndex(id: number): number {
		return id & this.choiceMask;
	}

	armIn(chunk: ChoiceChunk, index: number): Arm {
		const arm = at(chunk.arms, index);
		return {
			condition:
				arm.condition === null
					? null
					: this.conditionName(arm.condition),
			tag: this.text(arm.tag),
			annotations: this.annotationsIn(
				chunk.annotations,
				arm.annotations
			),
		};
	}

	private conditionName(id: number): string {
		return this.text(at(this.data.conditions, id));
	}

	/** Resolves a string ID; parser IDs always exist, so a miss is a corrupted graph. */
	text(id: number): string {
		return at(this.data.strings, id);
	}

	private annotationsIn(
		annotations: readonly AnnotationRecord[] | Table<AnnotationRecord>,
		span: Span
	): readonly Annotation[] {
		const result: Annotation[] = [];
		for (let i = span[0]; i < span[1]; i++) {
			const entry =
				annotations instanceof Table
					? annotations.get(i)
					: at(annotations, i);
			result.push({
				key: this.text(entry.key),
				value:
					entry.value === null
						? null
						: this.text(entry.value),
			});
		}
		return result;
	}
}

export function defineGraph<V extends Vocabulary>(data: GraphData): Graph<V> {
	return new Graph<V>(data);
}

/** The bitset byte holding condition `id`. */
function byteOf(id: number): number {
	return id >> 3;
}

/** The bit within `byteOf(id)` for condition `id`. */
function maskOf(id: number): number {
	return 1 << (id & 7);
}

/**
 * Brent's cycle detection over the nodes a run enters. `visit` returns true when the run
 * returns to the current checkpoint, which means it cycles without ending.
 */
class CycleDetector {
	private checkpoint = -1;
	private window = 1;
	private steps = 0;
	private visits = 0;

	/** Starts a new walk. Reuses this detector, so a step allocates nothing. */
	reset(): void {
		this.checkpoint = -1;
		this.window = 1;
		this.steps = 0;
		this.visits = 0;
	}

	visit(node: number): boolean {
		this.visits++;
		if (this.visits === 1) {
			this.checkpoint = node;
			return false;
		}
		this.steps++;
		if (node === this.checkpoint) {
			return true;
		}
		if (this.steps === this.window) {
			this.checkpoint = node;
			this.window *= 2;
			this.steps = 0;
		}
		return false;
	}
}

/**
 * One execution of a graph. Follows the Graphite execution model: gates in source order,
 * then the sequence edge only when `finish()` was called for the current node. Conditions
 * persist until `set` changes them. Ended and errored runs take no further transitions.
 */
export class Run<V extends Vocabulary> {
	private readonly graph: Graph<V>;
	private readonly bits: Uint8Array;
	private readonly cycle = new CycleDetector();
	private readonly stepResult = {
		node: undefined as unknown,
		transitions: 0,
		taken: 0,
		status: 'running' as Status,
		error: null as ErrorCode | null,
	};
	private nodeId = 0;
	private phase: Exclude<Status, 'error'> = 'running';
	private failure: ErrorCode | null = null;
	private finishedFlag = false;
	private transitionCount = 0;

	private readonly maxTransitions: number;

	constructor(graph: Graph<V>, options: RunOptions = {}) {
		this.graph = graph;
		this.bits = new Uint8Array(Math.ceil(graph.conditionCount / 8));
		this.maxTransitions =
			options.maxTransitions ?? Number.POSITIVE_INFINITY;
		if (graph.nodeRecord(0).end) {
			this.phase = 'ended';
		}
	}

	get node(): V['nodes'] {
		return this.graph.nodeName(this.nodeId);
	}

	/** The run's state as plain data. Store it as JSON and restore it with `restore`. */
	snapshot(): RunSnapshot {
		return {
			version: 1,
			graph: this.graph.fingerprint(),
			node: this.nodeId,
			phase: this.phase,
			failure: this.failure,
			finished: this.finishedFlag,
			transitions: this.transitionCount,
			bits: Array.from(this.bits),
		};
	}

	/**
	 * Rebuilds a run from a snapshot taken on the same graph. Throws a RangeError for a snapshot
	 * from another graph, from another version, or with state the graph cannot be in.
	 */
	static restore<V extends Vocabulary>(
		graph: Graph<V>,
		snapshot: RunSnapshot,
		options: RunOptions = {}
	): Run<V> {
		if (snapshot.version !== 1) {
			throw new RangeError(
				`unsupported snapshot version ${String(snapshot.version)}`
			);
		}
		if (snapshot.graph !== graph.fingerprint()) {
			throw new RangeError(
				`snapshot does not belong to graph "${graph.name}"`
			);
		}
		const run = new Run(graph, options);
		const valid =
			Number.isInteger(snapshot.node) &&
			snapshot.node >= 0 &&
			snapshot.node < graph.data.nodeCount &&
			Number.isInteger(snapshot.transitions) &&
			snapshot.transitions >= 0 &&
			(snapshot.phase === 'running' ||
				snapshot.phase === 'ended') &&
			(snapshot.failure === null ||
				RESTORABLE_FAILURES.includes(snapshot.failure)) &&
			snapshot.bits.length === run.bits.length &&
			snapshot.bits.every(
				(byte) =>
					Number.isInteger(byte) && byte >= 0 && byte <= 255
			);
		if (!valid) {
			throw new RangeError(
				`snapshot holds state that graph "${graph.name}" cannot be in`
			);
		}
		run.nodeId = snapshot.node;
		run.phase = snapshot.phase;
		run.failure = snapshot.failure;
		run.finishedFlag = snapshot.finished;
		run.transitionCount = snapshot.transitions;
		run.bits.set(snapshot.bits);
		return run;
	}

	get status(): Status {
		return this.failure !== null ? 'error' : this.phase;
	}

	get error(): ErrorCode | null {
		return this.failure;
	}

	get transitions(): number {
		return this.transitionCount;
	}

	set(condition: V['conditions'], value: boolean): void {
		this.setId(this.graph.conditionId(condition), value);
	}

	/** Hot-path variant of `set`, taking an ID from `Graph.conditionId`. */
	setId(id: number, value: boolean): void {
		if (
			!Number.isInteger(id) ||
			id < 0 ||
			id >= this.graph.conditionCount
		) {
			throw new RangeError(`condition ID ${id} is out of range`);
		}
		const byte = byteOf(id);
		const mask = maskOf(id);
		const current = this.bits[byte] ?? 0;
		this.bits[byte] = value ? current | mask : current & ~mask;
	}

	/** Whether condition `condition` is currently true. */
	holds(condition: V['conditions']): boolean {
		return this.isSet(this.graph.conditionId(condition));
	}

	/** Marks the current stage finished, enabling the node's sequence edge. No-op once ended or errored. */
	finish(): void {
		if (this.isRunning()) {
			this.finishedFlag = true;
		}
	}

	/** The first arm in source order whose condition holds, else the else arm, else undefined. */
	choose<C extends keyof V['choices'] & string>(
		choice: C
	): V['choices'][C] | undefined {
		const id = this.graph.choiceId(choice);
		const chunk = this.graph.choiceChunk(id);
		const span = at(chunk.choices, this.graph.choiceIndex(id)).arms;
		for (let index = span[0]; index < span[1]; index++) {
			const condition = at(chunk.arms, index).condition;
			if (condition === null || this.isSet(condition)) {
				return this.graph.armIn(
					chunk,
					index
				) as V['choices'][C];
			}
		}
		return undefined;
	}

	/**
	 * Takes transitions until none applies. With `trace`, each applied transition's edge ID is
	 * written to it; if it is full, the run errors with `output_full` before that transition.
	 */
	step(trace?: Uint32Array): StepResult<V> {
		let taken = 0;
		if (!this.isRunning()) {
			return this.result(taken);
		}
		this.cycle.reset();
		for (;;) {
			const edge = this.nextEdge();
			if (edge === undefined) {
				break;
			}
			if (trace !== undefined) {
				if (taken >= trace.length) {
					this.failure = 'output_full';
					break;
				}
				trace[taken] = edge.id;
			}
			taken++;
			this.commit(edge);
			if (this.phase === 'ended') {
				break;
			}
			if (this.cycle.visit(edge.target)) {
				this.failure = 'cycle';
				break;
			}
		}
		return this.result(taken);
	}

	/**
	 * Takes at most one transition and returns its edge ID, or -1 when none applies. Unlike `step`,
	 * it stops on the node it reaches, so the host can settle that node's task before any gate there
	 * reads an outcome. Conditions are still read by their gates, so the host must set them before
	 * each call. Loops are bounded by `maxTransitions`, not by the cycle detector.
	 */
	advance(): number {
		if (!this.isRunning()) {
			return -1;
		}
		const edge = this.nextEdge();
		if (edge === undefined) {
			return -1;
		}
		this.commit(edge);
		return edge.id;
	}

	/** The transition to take from the current node, or undefined. Sets `failure` at the limit. */
	private nextEdge(): EdgeRecord | undefined {
		const chunk = this.graph.nodeChunk(this.nodeId);
		const record = chunk.nodes.get(this.graph.nodeIndex(this.nodeId));
		const local = this.select(chunk, record);
		if (local < 0) {
			return undefined;
		}
		if (this.transitionCount >= this.maxTransitions) {
			this.failure = 'limit';
			return undefined;
		}
		return chunk.edges.get(local);
	}

	/** Moves to the edge's target and clears the stage's completion. Shared by `step` and `advance`. */
	private commit(edge: { readonly target: number }): void {
		this.transitionCount++;
		this.nodeId = edge.target;
		this.finishedFlag = false;
		if (this.graph.nodeRecord(edge.target).end) {
			this.phase = 'ended';
		}
	}

	private isRunning(): boolean {
		return this.phase === 'running' && this.failure === null;
	}

	/** The chunk-local index of the edge to take from `record`, or -1. */
	private select(chunk: NodeChunk, record: NodeRecord): number {
		for (
			let index = record.outgoing[0];
			index < record.outgoing[1];
			index++
		) {
			const condition = chunk.edges.get(index).condition;
			if (condition !== null && this.isSet(condition)) {
				return index;
			}
		}
		return this.finishedFlag && record.sequence !== null
			? record.sequence
			: -1;
	}

	private isSet(id: number): boolean {
		return ((this.bits[byteOf(id)] ?? 0) & maskOf(id)) !== 0;
	}

	/**
	 * The step result. It is the same object on every call, updated in place, so it is valid only
	 * until the next step. Copy the fields if they must outlive it.
	 */
	private result(taken: number): StepResult<V> {
		const out = this.stepResult;
		out.node = this.node;
		out.transitions = this.transitionCount;
		out.taken = taken;
		out.status = this.failure !== null ? 'error' : this.phase;
		out.error = this.failure;
		return out as unknown as StepResult<V>;
	}
}

/** Rebuilds a run from a snapshot. See `Run.restore`. */
export function restore<V extends Vocabulary>(
	graph: Graph<V>,
	snapshot: RunSnapshot,
	options?: RunOptions
): Run<V> {
	return Run.restore(graph, snapshot, options);
}

export function start<V extends Vocabulary>(
	graph: Graph<V>,
	options?: RunOptions
): Run<V> {
	return new Run(graph, options);
}
