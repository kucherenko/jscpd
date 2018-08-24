import { lstatSync, readFileSync, Stats } from 'fs';
import { Glob } from 'glob';
import { Detector } from './detector';
import { END_EVENT, END_PROCESS_EVENT, Events, INITIALIZE_EVENT } from './events';
import { getFormatByFile } from './formats';
import { IClone } from './interfaces/clone.interface';
import { IListener } from './interfaces/listener.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISource } from './interfaces/source.interface';
import { IStore } from './interfaces/store/store.interface';
import { getRegisteredListeners, registerListenerByName } from './listeners';
import { getRegisteredReporters, registerReportersByName } from './reporters';
import { CLONES_DB } from './stores/models';
import { StoresManager } from './stores/stores-manager';
import { getDefaultOptions } from './utils/options';

export class JSCPD {
  private detector: Detector;

  constructor(private options: IOptions) {
    this.options = { ...getDefaultOptions(), ...this.options };
    this.initializeListeners();
    this.initializeReporters();
    Events.emit(INITIALIZE_EVENT);
    this.detector = new Detector(this.options);
  }

  public detectInFiles(pathToFiles?: string): Promise<IClone[]> {
    return new Promise<IClone[]>(resolve => {
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
          if (source.split('\n').length >= this.options.minLines) {
            this.detect({
              id: path,
              source,
              format,
              detection_date: new Date().getTime(),
              last_update_date: fileStat.mtime.getTime()
            });
          }
        }
      });

      glob.on('end', () => {
        const clones: IClone[] = Object.values(StoresManager.getStore(CLONES_DB).getAll());
        Events.emit(END_EVENT, clones);
        resolve(clones);
      });
    }).then((clones: IClone[]) => {
      Events.emit(END_PROCESS_EVENT);
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
