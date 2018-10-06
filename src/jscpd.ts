import { stream } from 'fast-glob';
import { existsSync, lstatSync, readFileSync, Stats } from 'fs';
import { Detector } from './detector';
import {
  END_EVENT,
  END_GLOB_STREAM_EVENT,
  END_PROCESS_EVENT,
  FINISH_EVENT,
  INITIALIZE_EVENT,
  JscpdEventEmitter,
  JSCPDEventEmitter
} from './events';
import { getFormatByFile, getSupportedFormats } from './formats';
import { IClone } from './interfaces/clone.interface';
import { IListener } from './interfaces/listener.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISource } from './interfaces/source.interface';
import { IStore } from './interfaces/store/store.interface';
import { getRegisteredListeners, registerListenerByName } from './listeners';
import { getRegisteredReporters, registerReportersByName } from './reporters';
import { CLONES_DB } from './stores/models';
import { StoreManager, StoresManager } from './stores/stores-manager';
import { getDefaultOptions } from './utils/options';

const gitignoreToGlob = require('gitignore-to-glob');

export class JSCPD {
  public static getStoreManager(): StoreManager<any> {
    return StoresManager;
  }

  public static getEventsEmitter(): JscpdEventEmitter {
    return JSCPDEventEmitter;
  }

  public static emit(event: string | symbol, ...args: any[]): boolean {
    return JSCPD.getEventsEmitter().emit(event, ...args);
  }

  public static on(event: string | symbol, listener: (...args: any[]) => void): JscpdEventEmitter {
    return JSCPD.getEventsEmitter().on(event, listener);
  }

  public static getSupporterFormats(): string[] {
    return getSupportedFormats();
  }

  public static getDefaultOptions(): IOptions {
    return getDefaultOptions();
  }

  private detector: Detector;

  constructor(private options: IOptions) {
    this.options = { ...JSCPD.getDefaultOptions(), ...this.options };
    this.initializeListeners();
    this.initializeReporters();
    JSCPD.getEventsEmitter().emit(INITIALIZE_EVENT);
    this.detector = new Detector(this.options);
  }

  public detectInFiles(pathToFiles?: string): Promise<IClone[]> {
    let ignore: string[] = this.options.ignore || [];

    if (this.options.gitignore && existsSync(pathToFiles + '/.gitignore')) {
      ignore = [...ignore, ...gitignoreToGlob(pathToFiles + '/.gitignore')].map(pattern => pattern.replace('!', ''));
    }

    return new Promise<IClone[]>(resolve => {
      const glob = stream(['**/*'], {
        cwd: pathToFiles,
        ignore,
        onlyFiles: true,
        dot: true,
        absolute: true
      });

      glob.on('data', path => {
        const format: string = getFormatByFile(path, this.options.formatsExts) as string;
        if (format && this.options.format && this.options.format.includes(format)) {
          const fileStat: Stats = lstatSync(path);
          const source: string = readFileSync(path).toString();
          if (source.split('\n').length >= this.options.minLines) {
            this.detect({
              id: path,
              source,
              format,
              detectionDate: new Date().getTime(),
              lastUpdateDate: fileStat.mtime.getTime()
            });
          }
        }
      });

      glob.on('end', () => {
        JSCPD.emit(END_GLOB_STREAM_EVENT);
      });

      JSCPD.on(FINISH_EVENT, () => {
        const clones: IClone[] = Object.values(StoresManager.getStore(CLONES_DB).getAll());
        JSCPD.emit(END_EVENT, clones);
        resolve(clones);
      });
    }).then((clones: IClone[]) => {
      JSCPD.emit(END_PROCESS_EVENT);
      return clones;
    });
  }

  public detectBySource(source: ISource): IClone[] {
    this.detect(source);
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    return Object.values(clonesStore.getAll());
  }

  private detect(source: ISource) {
    this.detector.detect(source);
  }

  private initializeReporters() {
    registerReportersByName(this.options);

    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.attach();
    });
  }

  private initializeListeners() {
    registerListenerByName(this.options);

    Object.values(getRegisteredListeners()).map((listener: IListener) => {
      listener.attach();
    });
  }
}
