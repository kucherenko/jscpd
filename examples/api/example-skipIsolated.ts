import {detectClones} from "jscpd";

(async () => {
  const clones = await detectClones({
    path: [
      __dirname + '/../fixtures'
    ],
    skipIsolated: [
      ['packages/businessA', 'packages/businessB', 'packages/businessC'],
    ],
    silent: true
  });
  console.log(clones);
})()
