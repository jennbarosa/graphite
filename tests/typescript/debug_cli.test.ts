// Drives the debugger against the walk spec compiled by `tests/debugger_cli.rs`.
import { describe, expect, test } from 'bun:test';
import { openSession, type Session } from '../../cli/session';

const modulePath = process.env.GRAPHITE_DEBUG_WALK;
if (modulePath === undefined) {
	throw new Error('GRAPHITE_DEBUG_WALK must name the compiled walk module');
}
const walk: string = modulePath;

/** Runs each line in order and returns every output. */
function run(session: Session, ...lines: string[]): string[] {
	return lines.map((line) => session.execute(line) ?? 'QUIT');
}

describe('graphite debugger on the walk cycle', () => {
	test('a new session starts at the entry with nothing set', async () => {
		const session = await openSession(walk);
		expect(session.execute('show')).toBe(
			'node windup  status running  transitions 0  true: none'
		);
	});

	test('a sequence edge waits for finish', async () => {
		const session = await openSession(walk);
		const [, stepped] = run(session, 'step', 'step');
		expect(stepped).toContain('node windup');
		expect(stepped).toContain('no edges taken');
	});

	test('finish then step moves into the walk loop', async () => {
		const session = await openSession(walk);
		const [, stepped] = run(session, 'finish', 'step');
		expect(stepped!.split('\n')[0]).toBe(
			'node going  status running  transitions 1  true: none'
		);
		expect(stepped!.split('\n')[1]).toMatch(/^edges \d+$/);
	});

	test('a gate beats the sequence edge when its condition holds', async () => {
		const session = await openSession(walk);
		const [, , stepped] = run(
			session,
			'set keyup on',
			'finish',
			'step'
		);
		expect(stepped!.split('\n')[0]).toBe(
			'node stopped  status ended  transitions 1  true: keyup'
		);
	});

	test('keyup while walking winds down and ends', async () => {
		const session = await openSession(walk);
		const lines = run(
			session,
			'finish',
			'step',
			'set keyup on',
			'step'
		);
		expect(lines.at(-1)!.split('\n')[0]).toBe(
			'node winddown  status ended  transitions 2  true: keyup'
		);
	});

	test('an ended run takes no further steps', async () => {
		const session = await openSession(walk);
		run(session, 'set keyup on', 'finish', 'step');
		expect(session.execute('step')).toContain('no edges taken');
	});

	test('choose falls back to else when no condition holds', async () => {
		const session = await openSession(walk);
		expect(session.execute('choose facing')).toBe(
			'facing: else -> down'
		);
	});

	test('choose returns the first true arm with its annotations', async () => {
		const session = await openSession(walk);
		run(session, 'set left on');
		expect(session.execute('choose facing')).toBe(
			'facing: when left -> side @flip:true'
		);
	});

	test('choose does not change the run', async () => {
		const session = await openSession(walk);
		run(session, 'set left on', 'choose facing');
		expect(session.execute('show')).toBe(
			'node windup  status running  transitions 0  true: left'
		);
	});

	test('a condition set on and then off is no longer selected', async () => {
		const session = await openSession(walk);
		run(session, 'set up on', 'set up off');
		expect(session.execute('choose facing')).toBe(
			'facing: else -> down'
		);
	});

	test('history lists the edges taken, and reset clears them', async () => {
		const session = await openSession(walk);
		expect(session.execute('history')).toBe('no edges taken');
		run(session, 'finish', 'step');
		expect(session.execute('history')).toMatch(/^\d+$/);
		expect(session.execute('reset')).toContain('node windup');
		expect(session.execute('history')).toBe('no edges taken');
	});

	test('reset turns conditions back off', async () => {
		const session = await openSession(walk);
		run(session, 'set keyup on', 'reset');
		expect(session.execute('show')).toBe(
			'node windup  status running  transitions 0  true: none'
		);
	});

	test('unknown names and bad arguments are reported, not thrown', async () => {
		const session = await openSession(walk);
		expect(session.execute('set nosuch on')).toMatch(
			/^error: unknown condition "nosuch"/
		);
		expect(session.execute('choose nosuch')).toMatch(
			/^error: unknown choice "nosuch"/
		);
		expect(session.execute('set keyup')).toBe(
			'usage: set <condition> on|off'
		);
		expect(session.execute('choose')).toBe('usage: choose <choice>');
		expect(session.execute('frobnicate')).toBe(
			'unknown command "frobnicate"; try help'
		);
	});

	test('quit ends the session', async () => {
		const session = await openSession(walk);
		expect(session.execute('quit')).toBeNull();
	});
});
