// Graphite graph debugger.
//
//   bun cli/debug.ts <dir>/<graph>.ts [command...]
//
// `<dir>` is an output directory from `graphite compile`. With commands, runs them in order;
// without, reads commands from stdin.
import { createInterface } from 'node:readline';
import { openSession } from './session';

async function main(argv: string[]): Promise<number> {
	const [modulePath, ...commands] = argv;
	if (modulePath === undefined) {
		console.error(
			'usage: bun cli/debug.ts <dir>/<graph>.ts [command...]'
		);
		return 2;
	}
	const session = await openSession(modulePath);
	if (commands.length > 0) {
		for (const command of commands) {
			const output = session.execute(command);
			if (output === null) break;
			console.log(output);
		}
		return 0;
	}
	const prompt = process.stdin.isTTY ? 'graphite> ' : '';
	const lines = createInterface({ input: process.stdin, terminal: false });
	process.stdout.write(prompt);
	for await (const line of lines) {
		const output = session.execute(line);
		if (output === null) break;
		if (output !== '') console.log(output);
		process.stdout.write(prompt);
	}
	return 0;
}

// The entry point. Nothing imports this file, so it runs `main` directly. It avoids top-level
// `await` and `import.meta`, which CommonJS builds such as `tsx` without `"type": "module"` reject.
main(process.argv.slice(2)).then((code) => {
	process.exitCode = code;
});
