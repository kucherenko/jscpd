import {lstatSync, readFileSync, Stats} from 'fs';
import {Glob} from 'glob';
import {Detector} from './detector';
import {Events} from './events';
import {getFormatByFile, getSupportedFormats} from './formats';
import {IClone} from './interfaces/clone.interface';
import {IOptions} from './interfaces/options.interface';
import {IReporter} from './interfaces/reporter.interface';
import {ISource} from './interfaces/source.interface';
import {getRegisteredReporters, registerReportersByName} from './reporters';
import {StoresManager} from './stores/stores-manager';

export class JSCPD {
  private detector: Detector;

  constructor(private options: IOptions) {
    StoresManager.initialize(this.options.storeOptions);
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
          Events.emit('match', {path, format, source});
          clones = clones.concat(
            ...this.detect({
              id: path,
              source,
              format,
              last_update: new Date(fileStat.mtime).getMilliseconds(),
              size: fileStat.size
            })
          );
        }
      });

      glob.on('error', (...args: any[]) => {
        glob.abort();
        rejects(args);
      });

      glob.on('end', () => {
        Events.emit('end', clones);
        resolve(clones);
      });
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
      'clones',
      'source',
      'statistic',
      ...getSupportedFormats().map(name => `hashes.${name}`)
    ]);
  }

  private initializeReporters() {
    registerReportersByName(this.options);
    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.attach();
    });
  }
}
