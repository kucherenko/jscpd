import { stream } from 'fast-glob';
import { existsSync, lstatSync, readFileSync, Stats } from 'fs';
import { getSourceFragmentLength } from './clone';
import { Detector } from './detector';
import {
  END_EVENT,
  END_GLOB_STREAM_EVENT,
  END_PROCESS_EVENT,
  FINISH_EVENT,
  INITIALIZE_EVENT,
  JscpdEventEmitter,
  MATCH_SOURCE_EVENT
} from './events';
import { IClone } from './interfaces/clone.interface';
import { IListener } from './interfaces/listener.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISource } from './interfaces/source.interface';
import { IStore } from './interfaces/store/store.interface';
import { IToken } from './interfaces/token/token.interface';
import { getRegisteredListeners, registerListenerByName } from './listeners';
import { getModeHandler } from './modes';
import { getRegisteredReporters, registerReportersByName } from './reporters';
import { CLONES_DB } from './stores/models';
import { StoreManager, StoresManager } from './stores/stores-manager';
import EventEmitter = NodeJS.EventEmitter;
import { createTokensMaps, tokenize } from './tokenizer';
import { getFormatByFile } from './tokenizer/formats';
import { TokensMap } from './tokenizer/token-map';
import { generateSourceId } from './utils';
import { getDefaultOptions } from './utils/options';
import { timerStart, timerStop } from './utils/timer';

const gitignoreToGlob = require('gitignore-to-glob');

export function getStoreManager(): StoreManager<any> {
  return StoresManager;
}

export class JSCPD {
  private detector: Detector;
  private readonly eventEmitter: JscpdEventEmitter;

  constructor(private readonly options: IOptions, eventEmitter?: EventEmitter) {
    timerStart(this.constructor.name + '::constructor');
    this.eventEmitter = eventEmitter || new JscpdEventEmitter();
    this.options = { ...getDefaultOptions(), ...this.options };
    this.initializeListeners();
    this.initializeReporters();
    this.detector = new Detector(this.options, this.eventEmitter);
    this.eventEmitter.emit(INITIALIZE_EVENT);
    timerStop(this.constructor.name + '::constructor');
  }

  public detectInFiles(pathToFiles?: string): Promise<IClone[]> {
    let ignore: string[] = this.options.ignore || [];

    if (this.options.gitignore && existsSync(pathToFiles + '/.gitignore')) {
      ignore = [...ignore, ...gitignoreToGlob(pathToFiles + '/.gitignore')].map(pattern => pattern.replace('!', ''));
    }

    return new Promise<IClone[]>(resolve => {
      timerStart('glob-init');
      const glob = stream(['**/*'], {
        cwd: pathToFiles,
        ignore,
        onlyFiles: true,
        dot: true,
        absolute: true
      });
      timerStop('glob-init');

      glob.on('data', path => {
        const format: string = getFormatByFile(path, this.options.formatsExts) as string;
        if (format && this.options.format && this.options.format.includes(format)) {
          timerStart('read-file');
          const fileStat: Stats = lstatSync(path);
          const source: string = readFileSync(path).toString();
          const lines = source.split('\n').length;
          timerStop('read-file');
          if (lines >= this.options.minLines) {
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
        this.eventEmitter.emit(END_GLOB_STREAM_EVENT);
      });

      this.eventEmitter.on(FINISH_EVENT, () => {
        const clones: IClone[] = Object.values(StoresManager.getStore(CLONES_DB).getAll());
        this.eventEmitter.emit(END_EVENT, clones);
        resolve(clones);
      });
    }).then((clones: IClone[]) => {
      this.eventEmitter.emit(END_PROCESS_EVENT);
      return clones;
    });
  }

  public getAllClones(): IClone[] {
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    return Object.values(clonesStore.getAll());
  }

  public detect(source: ISource): IClone[] {
    let clones: IClone[] = [];
    timerStart('tokenize');
    const tokens: IToken[] = tokenize(source.source, source.format).filter(getModeHandler(this.options.mode));
    timerStop('tokenize');

    timerStart('createTokenMap');
    const tokenMaps: TokensMap[] = createTokensMaps(tokens, this.options.minTokens).map(tokenMap => {
      const subSource: ISource = {
        ...source,
        format: tokenMap.getFormat(),
        range: [tokenMap.getStartPosition(), tokenMap.getEndPosition()],
        lines: getSourceFragmentLength(source, tokenMap.getStartPosition(), tokenMap.getEndPosition())
      };
      tokenMap.setSourceId(generateSourceId(subSource));
      this.eventEmitter.emit(MATCH_SOURCE_EVENT, subSource);
      return tokenMap;
    });
    timerStop('createTokenMap');

    tokenMaps.forEach((tokenMap: TokensMap) => {
      timerStart('detect');
      clones = clones.concat(this.detector.detectByMap(tokenMap));
      timerStop('detect');
    });

    return clones;
  }

  private initializeReporters() {
    registerReportersByName(this.options);

    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.attach(this.eventEmitter);
    });
  }

  private initializeListeners() {
    registerListenerByName(this.options);

    Object.values(getRegisteredListeners()).map((listener: IListener) => {
      listener.attach(this.eventEmitter);
    });
  }
}
