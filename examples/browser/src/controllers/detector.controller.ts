import {Controller} from "stimulus";
import {Tokenizer} from '@jscpd/tokenizer';
import {
  Detector,
  DetectorEvents,
  IClone, IHandler,
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
    "sources",
    "lines",
    "clones",
    "clonedLines",
    "percent",
  ]

  formatTarget: HTMLInputElement;
  codeTarget: HTMLInputElement;
  sourcesTarget: HTMLElement;
  linesTarget: HTMLElement;
  clonesTarget: HTMLElement;
  clonedLinesTarget: HTMLElement;
  percentTarget: HTMLElement;

  tokenizer: ITokenizer;
  store: IStore<IMapFrame>;
  detector: Detector;
  statistic: Statistic;

  validators = []
  options: IOptions = {
    minLines: 5,
    maxLines: 500,
    minTokens: 50,
    mode: weak,
  }

  initialize() {
    this.tokenizer = new Tokenizer();
    this.store = new MemoryStore();
    this.statistic = new Statistic(this.options);
    this.detector = new Detector(this.tokenizer, this.store, this.validators, this.options);

    Object
      .entries(this.statistic.subscribe())
      .map(([event, handler]: [DetectorEvents, IHandler]) => this.detector.on(event, handler));
  }

  async detect() {

    const format = 'javascript';
    const code: string = this.codeTarget.value;

    const clones: IClone[] = await this.detector.detect('source id ' + (new Date()).toISOString(), code, format);
    const {total} = this.statistic.getStatistic()
    this.linesTarget.innerText = total.lines.toString();
    this.sourcesTarget.innerText = total.sources.toString();
    this.clonesTarget.innerText = total.clones.toString();
    this.clonedLinesTarget.innerText = total.duplicatedLines.toString();
    this.percentTarget.innerText = total.percentage.toString();
    console.log(clones);
    console.log(this.sourcesTarget);
  }
}
