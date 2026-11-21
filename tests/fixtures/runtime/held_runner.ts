// A task runner that leaves a timer open, as a database pool or keep-alive client would. The CLI
// must still exit when the run ends.
setInterval(() => {}, 1000);

export default async function (task: string) {
	return 'pass' as const;
}
