// A task runner for `graphite run --durable --runner`: every task passes.
export default async function (task: string) {
	console.error(`ran ${task}`);
	return 'pass' as const;
}
