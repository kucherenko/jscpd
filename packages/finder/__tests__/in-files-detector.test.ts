import {InFilesDetector} from "../src/in-files-detector";
import {expect} from "chai";
import {Tokenizer} from "@jscpd/tokenizer";
import {MemoryStore, Statistic} from "@jscpd/core";

describe('jscpd finder: in-files-detector', () => {
  it('should not detect for empty files list', async () => {
    const detector = new InFilesDetector(new Tokenizer(), new MemoryStore(), new Statistic({}), {});
    const detected = await detector.detect([]);
    expect(detected).to.deep.equal([]);
  });
})
