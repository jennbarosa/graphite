// Shapes the generated graph module and its runtime provide. The debugger depends only on these.

export interface Annotation {
	readonly key: string;
	readonly value: string | null;
}

export interface Arm {
	readonly condition: string | null;
	readonly tag: string;
	readonly annotations: readonly Annotation[];
}

export interface StepLike {
	readonly node: string;
	readonly status: string;
	readonly error: string | null;
	readonly transitions: number;
}

export interface RunLike extends StepLike {
	set(condition: string, value: boolean): void;
	holds(condition: string): boolean;
	choose(choice: string): Arm | undefined;
	finish(): void;
	step(trace?: Uint32Array): StepLike & { readonly taken: number };
}

export interface GraphLike {
	readonly name: string;
	conditionNames(): string[];
}
