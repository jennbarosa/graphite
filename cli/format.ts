// Text the debugger prints. Pure functions of their inputs, so they hold no session state.
import type { Annotation, Arm, StepLike } from './types';

export const HELP = `commands:
  show                 node, status, transitions, true conditions
  set <cond> on|off    set a condition's value
  finish               mark the current stage finished
  step                 settle the graph once; prints the edges taken
  choose <choice>      show the arm the choice selects now
  history              list every edge taken since start or reset
  reset                start a new run; conditions go back to off
  help                 this list
  quit                 leave`;

export const USAGE_SET = 'usage: set <condition> on|off';
export const USAGE_CHOOSE = 'usage: choose <choice>';

export function formatState(
	run: StepLike,
	trueConditions: readonly string[]
): string {
	const error = run.error === null ? '' : ` (${run.error})`;
	const list =
		trueConditions.length === 0 ? 'none' : trueConditions.join(', ');
	return `node ${run.node}  status ${run.status}${error}  transitions ${run.transitions}  true: ${list}`;
}

export function formatStep(state: string, edges: readonly number[]): string {
	const edgeText =
		edges.length === 0 ? 'no edges taken' : `edges ${edges.join(' ')}`;
	return `${state}\n${edgeText}`;
}

export function formatArm(choice: string, arm: Arm): string {
	const label = arm.condition === null ? 'else' : `when ${arm.condition}`;
	return `${choice}: ${label} -> ${arm.tag}${formatAnnotations(arm.annotations)}`;
}

function formatAnnotations(annotations: readonly Annotation[]): string {
	return annotations
		.map((entry) =>
			entry.value === null
				? ` @${entry.key}`
				: ` @${entry.key}:${entry.value}`
		)
		.join('');
}
