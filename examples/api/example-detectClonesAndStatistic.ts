import {detectClonesAndStatistic} from "jscpd";

(async () => {
  const data = await detectClonesAndStatistic({
    path: [
      __dirname + '/../fixtures'
    ],
    silent: true
  });
  console.log(data);
})()
