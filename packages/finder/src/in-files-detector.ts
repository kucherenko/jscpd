import {
  Detector,
  DetectorEvents,
  IClone,
  ICloneValidator,
  IHandler,
  IMapFrame,
  IOptions,
  IStore,
  ISubscriber,
  ITokenizer,
  Statistic,
} from '@jscpd/core';
import {getFormatByFile} from '@jscpd/tokenizer';
import {EntryWithContent, IHook, IReporter} from './interfaces';
import {SkipLocalValidator} from './validators';

export class InFilesDetector {

  private readonly reporters: IReporter[] = [];
  private readonly subscribes: ISubscriber[] = [];
  private readonly postHooks: IHook[] = [];

  constructor(
    private readonly tokenizer: ITokenizer,
    private readonly store: IStore<IMapFrame>,
    private readonly statistic: Statistic,
    public readonly options: IOptions) {
    this.registerSubscriber(this.statistic);
  }

  registerReporter(reporter: IReporter): void {
    this.reporters.push(reporter);
  }

  registerSubscriber(subscriber: ISubscriber): void {
    this.subscribes.push(subscriber);
  }

  registerHook(hook: IHook): void {
    this.postHooks.push(hook);
  }

  detect(fls: EntryWithContent[]): Promise<IClone[]> {
    const files = fls.filter((f) => !!f);
    if (files.length === 0) {
      return Promise.resolve([]);
    }
    const options = this.options;
    const hooks = [...this.postHooks];
    const store = this.store;
    const validators: ICloneValidator[] = [];

    if (options.skipLocal) {
      validators.push(new SkipLocalValidator());
    }

    const detector = new Detector(this.tokenizer, store, validators, options);

    this.subscribes.forEach((listener: ISubscriber) => {
      Object
        .entries(listener.subscribe())
        .map(([event, handler]: [DetectorEvents, IHandler]) => detector.on(event, handler));
    });

    const detect = (entry: EntryWithContent, clones: IClone[] = []): Promise<IClone[]> => {
      const {path, content} = entry;
      const format: string = getFormatByFile(path, options.formatsExts);
      return detector
        .detect(path, content, format)
        .then((clns: IClone[]) => {
          clones.push(...clns);
          const file = files.pop();
          if (file) {
            return detect(file, clones);
          }
          return clones;
        });
    };

    const processHooks = (hook: IHook, detectedClones: IClone[]): Promise<IClone[]> => {
      return hook
        .process(detectedClones)
        .then((clones: IClone[]) => {
          const nextHook: IHook = hooks.pop();
          if (nextHook) {
            return processHooks(nextHook, clones);
          }
          return clones;
        });
    }

    return detect(files.pop())
      .then((clones: IClone[]) => {
        const hook = hooks.pop();
        if (hook) {
          return processHooks(hook, clones)
        }
        return clones;
      })
      .then((clones: IClone[]) => {
        const statistic = this.statistic.getStatistic();
        this.reporters.forEach((reporter: IReporter) => {
          reporter.report(clones, statistic);
        });
        return clones;
      });
  }
}
