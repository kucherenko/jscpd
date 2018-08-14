import {Detector} from './detector';
import {IOptions} from './interfaces/options.interface';
import {getRegisteredReporters, registerReportersByName} from "./reporters";
import {ISource} from "./interfaces/source.interface";
import {IClone} from "./interfaces/clone.interface";
import {Glob} from "glob";
import {lstatSync, readFileSync, Stats} from "fs";
import {getFormatByFile, getSupportedFormats} from "./formats";
import {Events} from "./events";
import {IReporter} from "./interfaces/reporter.interface";
import {StoresManager} from "./stores/stores-manager";

export class JSCPD {

  private detector: Detector;

  constructor(private options: IOptions) {
    this.initializeReporters();
    this.detector = new Detector(this.options);
  }

  async detectInFiles(pathToFiles?: string): Promise<IClone[]> {
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
        if (format && this.options.format && this.options.format.includes(format)) {
          Events.emit('match', {path, format});
          const fileStat: Stats = lstatSync(path);
          const source: string = readFileSync(path).toString();
          clones = clones.concat(
            ...this.detect({
              id: path,
              source,
              format,
              last_update: (new Date(fileStat.mtime)).getMilliseconds(),
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

  async detectBySource(source: ISource) {
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
      ...getSupportedFormats().map(name => `hashes.${name}`)
    ]);
  }

  private initializeReporters() {
    registerReportersByName(this.options);
    Object
      .values(getRegisteredReporters())
      .map((reporter: IReporter) => {
        reporter.attach();
      });
  }
}
