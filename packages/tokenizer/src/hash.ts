import SparkMD5 from "spark-md5";

export function hash(value: string): string {
	return SparkMD5.hash(value);
}
