import {jscpd} from "../src";

(async () => {
  try {
    await jscpd(process.argv, process.exit)
  } catch(e) {
    console.log(e);
    process.exit(1);
  }
})()
