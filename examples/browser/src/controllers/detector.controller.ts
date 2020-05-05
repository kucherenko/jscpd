import {Controller} from "stimulus";
import {Tokenizer} from '@jscpd/tokenizer';
import {Detector, IClone, ICloneValidator, IMapFrame, IOptions, IStore, ITokenizer, MemoryStore} from '@jscpd/core';

export class DetectorController extends Controller {
  static targets = [
    "format",
    "code",
  ]

  formatTarget: HTMLInputElement;
  codeTarget: HTMLInputElement;

  tokenizer: ITokenizer;
  store: IStore<IMapFrame>;
  detector: Detector;
  validators: ICloneValidator[]
  options: IOptions = {
    minLines: 5,
    maxLines: 500,
  }

  initialize() {
    this.tokenizer = new Tokenizer();
    this.store = new MemoryStore();

    this.detector = new Detector(
      this.tokenizer,
      this.store,
      this.validators,
      this.options,
    );
  }

  async detect() {
    const format = this.formatTarget.value;
    const code: string = this.codeTarget.value;
    const clones: IClone[] = await this.detector.detect('source_id', code, format);
    console.log(clones);
  }
}
