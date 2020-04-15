import {createHash} from "crypto";

export function hash(value: string): string {
	return createHash('md5')
		.update(value)
		.digest('hex');
}
