import {Controller} from "stimulus";
// import * as Prism from 'prismjs';
import {Tokenizer} from '@jscpd/tokenizer';
import {
  Detector,
  DetectorEvents,
  IClone,
  ICloneValidator,
  IMapFrame,
  IOptions,
  IStore,
  ITokenizer,
  MemoryStore,
  Statistic,
  weak,
} from '@jscpd/core';

export class DetectorController extends Controller {
  static targets = [
    "format",
    "code",
  ]

  formatTarget: HTMLInputElement;
  codeTarget: HTMLInputElement;

  statistic: Statistic;
  tokenizer: ITokenizer;
  store: IStore<IMapFrame>;
  detector: Detector;
  validators: ICloneValidator[]
  options: IOptions = {
    minLines: 5,
    maxLines: 500,
    mode: weak,
  }

  initialize() {
    this.tokenizer = new Tokenizer();
    this.store = new MemoryStore();
    this.statistic = new Statistic(this.options);

    this.detector = new Detector(
      this.tokenizer,
      this.store,
      this.validators,
      this.options,
    );
    Object
      .entries(this.statistic.subscribe())
      .forEach(([topic, handler]) => this.detector.on(topic as DetectorEvents, handler))
  }

  async detect() {
    const format = this.formatTarget.value;
    const code: string = this.codeTarget.value;
    const clones: IClone[] = await this.detector.detect('source_id', code, format);
    console.log(clones);
    console.log(this.store);
  }
}
