// A task command for the command-task tests. It reads one request from stdin and answers as its
// mode (argv[2]) says: echo the request, print nothing, fail, exit 3, print junk, or hang.
const mode = process.argv[2];
const request = JSON.parse(await Bun.stdin.text());
switch (mode) {
	case 'echo':
		console.log(JSON.stringify({ outcome: 'pass', detail: { message: 'echo', data: request } }));
		break;
	case 'quiet':
		break;
	case 'fail':
		console.log(JSON.stringify({ outcome: 'fail', detail: { message: 'bad', code: 'x' } }));
		break;
	case 'exit':
		process.exit(3);
	case 'junk':
		console.log('not json');
		break;
	case 'hang':
		await new Promise(() => {});
		break;
}
