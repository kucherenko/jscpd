import { lstatSync, readFileSync, Stats } from 'fs';
import { Glob } from 'glob';
import { Detector } from './detector';
import { END_EVENT, END_PROCESS_EVENT, ERROR_EVENT, Events } from './events';
import { getFormatByFile, getSupportedFormats } from './formats';
import { IClone } from './interfaces/clone.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISource } from './interfaces/source.interface';
import { getRegisteredReporters, registerReportersByName } from './reporters';
import {
  CLONES_DB,
  getHashDbName,
  SOURCES_DB,
  STATISTIC_DB
} from './stores/models';
import { StoresManager } from './stores/stores-manager';

export class JSCPD {
  private detector: Detector;

  constructor(private options: IOptions) {
    StoresManager.initialize(this.options.storeOptions);
    Events.on(END_PROCESS_EVENT, () => StoresManager.close());
    this.initializeReporters();
    this.detector = new Detector(this.options);
  }

  public async detectInFiles(pathToFiles?: string): Promise<IClone[]> {
    await this.connectToStores();

    return new Promise<IClone[]>((resolve, rejects) => {
      let clones: IClone[] = [];
      const glob = new Glob('**/*', {
        cwd: pathToFiles,
        ignore: this.options.ignore,
        nodir: true,
        absolute: true
      });

      glob.on('match', path => {
        const format: string = getFormatByFile(path) as string;
        if (
          format &&
          this.options.format &&
          this.options.format.includes(format)
        ) {
          const fileStat: Stats = lstatSync(path);
          const source: string = readFileSync(path).toString();
          clones = clones.concat(
            ...this.detect({
              id: path,
              source,
              format,
              last_update: new Date(fileStat.mtime).getMilliseconds(),
              clones: [],
              hashes: {}
            })
          );
        }
      });

      glob.on('error', (...args: any[]) => {
        glob.abort();
        Events.emit(ERROR_EVENT, args);
        rejects(args);
      });

      glob.on('end', () => {
        Events.emit(END_EVENT, clones);
        resolve(clones);
      });
    }).then((clones: IClone[]) => {
      Events.emit(END_PROCESS_EVENT);
      return clones;
    });
  }

  public async detectBySource(source: ISource) {
    await this.connectToStores();
    return this.detect(source);
  }

  private detect(source: ISource) {
    return this.detector.detect(source);
  }

  private async connectToStores() {
    await StoresManager.connect([
      CLONES_DB,
      SOURCES_DB,
      STATISTIC_DB,
      ...getSupportedFormats().map(name => getHashDbName(name))
    ]);
  }

  private initializeReporters() {
    registerReportersByName(this.options);
    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.attach();
    });
  }
}
