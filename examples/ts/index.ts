import {detectClones} from "jscpd";
import {IMapFrame, MemoryStore} from "@jscpd/core";

(async () => {
  const store = new MemoryStore<IMapFrame>();

  await detectClones({
    path: [
      '/../fixtures'
    ],
  }, store);

  await detectClones({
    path: [
       '/../fixtures'
    ],
    silent: false
  }, store);
})()
