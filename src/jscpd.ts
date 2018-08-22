import { lstatSync, readFileSync, Stats } from 'fs';
import { Glob } from 'glob';
import { Detector } from './detector';
import { END_EVENT, END_PROCESS_EVENT, ERROR_EVENT, Events, INITIALIZE_EVENT } from './events';
import { getFormatByFile } from './formats';
import { IClone } from './interfaces/clone.interface';
import { IListener } from './interfaces/listener.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISource } from './interfaces/source.interface';
import { getRegisteredListeners, registerListenerByName } from './listeners';
import { getRegisteredReporters, registerReportersByName } from './reporters';

export class JSCPD {
  private detector: Detector;

  constructor(private options: IOptions) {
    this.initializeListeners();
    this.initializeReporters();
    Events.emit(INITIALIZE_EVENT);
    this.detector = new Detector(this.options);
  }

  public detectInFiles(pathToFiles?: string): Promise<IClone[]> {
    return new Promise<IClone[]>((resolve, rejects) => {
      let clones: IClone[] = [];
      const glob = new Glob('**/*', {
        cwd: pathToFiles,
        ignore: this.options.ignore,
        nodir: true,
        absolute: true
      });

      glob.on('match', path => {
        const format: string = getFormatByFile(path, this.options.formatsExts) as string;
        if (format && this.options.format && this.options.format.includes(format)) {
          const fileStat: Stats = lstatSync(path);
          const source: string = readFileSync(path).toString();
          clones = clones.concat(
            ...this.detect({
              id: path,
              source,
              format,
              meta: {
                detection_date: (new Date()).getTime(),
                last_update_date: fileStat.mtime.getTime(),
                clones: [],
                hashes: {}
              }
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

  public detectBySource(source: ISource): IClone[] {
    return this.detect(source);
  }

  private detect(source: ISource): IClone[] {
    return this.detector.detect(source);
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
