import { runServer } from "../src";

(async () => {
  try {
    await runServer(process.argv, process.exit)
  } catch(e) {
    console.error(e);
    process.exit(1);
  }
})()

export * from '../src'
