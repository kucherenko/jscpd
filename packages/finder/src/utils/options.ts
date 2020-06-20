export function parseFormatsExtensions(extensions = ''): { [key: string]: string[] } | undefined {
	const result: { [key: string]: string[] } = {};

	if (!extensions) {
		return undefined;
	}

	extensions.split(';').forEach((format: string) => {
		const pair = format.split(':');
		result[pair[0]] = pair[1].split(',');
	});

	return result;
}
