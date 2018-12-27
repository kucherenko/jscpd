import bytes = require('bytes');
import { bold } from 'colors/safe';
import EventEmitter = require('eventemitter3');
import { sync } from 'fast-glob';
import { EntryItem } from 'fast-glob/out/types/entries';
import { existsSync } from 'fs';
import { getSourceFragmentLength } from './clone';
import { Detector } from './detector';
import { END_EVENT, JscpdEventEmitter, MATCH_SOURCE_EVENT, SOURCE_SKIPPED_EVENT } from './events';
import { IClone } from './interfaces/clone.interface';
import { IHook } from './interfaces/hook.interface';
import { IListener } from './interfaces/listener.interface';
import { IOptions } from './interfaces/options.interface';
import { IReporter } from './interfaces/reporter.interface';
import { ISourceOptions } from './interfaces/source-options.interface';
import { IStatistic } from './interfaces/statistic.interface';
import { IToken } from './interfaces/token/token.interface';
import { getRegisteredListeners, registerListenerByName } from './listeners';
import { getModeHandler } from './modes';
import { getRegisteredReporters, registerReportersByName } from './reporters';
import { STATISTIC_DB } from './stores/models';
import { StoreManager, StoresManager } from './stores/stores-manager';
import { createTokensMaps, initLanguages, tokenize } from './tokenizer';
import { getFormatByFile } from './tokenizer/formats';
import { TokensMap } from './tokenizer/token-map';
import { getDefaultOptions, getOption } from './utils/options';
import { sourceToString } from './utils/source';

const gitignoreToGlob = require('gitignore-to-glob');

export function getStoreManager(): StoreManager<any> {
  return StoresManager;
}

export class JSCPD {
  get options(): IOptions {
    return this._options;
  }

  set options(value: IOptions) {
    this._options = value;
  }

  get files(): EntryItem[] {
    return this._files;
  }

  set files(value: EntryItem[]) {
    this._files = value;
  }

  get clones(): IClone[] {
    return this._clones;
  }

  set clones(value: IClone[]) {
    this._clones = value;
  }

  private readonly eventEmitter: JscpdEventEmitter;
  private detector: Detector;
  private _options: IOptions;
  private _files: EntryItem[] = [];
  private _clones: IClone[] = [];
  private _preHooks: IHook[] = [];
  private _postHooks: IHook[] = [];

  constructor(options: IOptions = {} as IOptions, eventEmitter?: EventEmitter) {
    this.eventEmitter = eventEmitter || new JscpdEventEmitter();
    this._options = { ...getDefaultOptions(), ...options };
    this.initializeListeners();
    this.initializeReporters();
    this.detector = new Detector(this._options, this.eventEmitter);
    StoresManager.initialize(this._options.storeOptions);
  }

  public attachPreHook(hook: IHook) {
    this._preHooks.push(hook);
  }

  public attachPostHook(hook: IHook) {
    this._postHooks.push(hook);
  }

  public async detect(code: string, options: ISourceOptions): Promise<IClone[]> {
    await Promise.all(this._preHooks.map((hook: IHook) => hook.use(this)));
    this._clones = await this._detect(code, { ...options, source: code });
    await this._detectionFinished(this._clones);
    return this._clones;
  }

  public async detectInFiles(pathToFiles: string[] = []): Promise<IClone[]> {
    const ignore: string[] = this._options.ignore || [];

    if (this._options.gitignore && existsSync(pathToFiles + '/.gitignore')) {
      let gitignorePatterns: string[] = gitignoreToGlob(pathToFiles + '/.gitignore') || [];
      gitignorePatterns = gitignorePatterns.map(
        pattern => (pattern.substr(pattern.length - 1) === '/' ? `${pattern}**/*` : pattern)
      );
      ignore.push(...gitignorePatterns);
      ignore.map(pattern => pattern.replace('!', ''));
    }

    this._files = sync(
      pathToFiles.map(path => (path.substr(path.length - 1) === '/' ? `${path}**/*` : `${path}/**/*`)),
      {
        ignore,
        onlyFiles: true,
        dot: true,
        stats: true,
        absolute: true
      }
    );

    this._files = this._files.filter((stats: any) => {
      const { path } = stats;
      const format: string = getFormatByFile(path, this._options.formatsExts) as string;
      return format && this._options.format && this._options.format.includes(format);
    });

    if (this._options.debug) {
      console.log(bold(`Found ${this._files.length} files to detect.`));
    }

    await Promise.all(this._preHooks.map((hook: IHook) => hook.use(this)));

    const sources: Array<{ source: string; sourceOptions: ISourceOptions }> = this._files
      .filter((stats: any) => {
        const { path } = stats;
        if (this._options.debug) {
          console.log(path);
          return false;
        }
        if (stats.size > bytes(getOption('maxSize', this._options))) {
          this.eventEmitter.emit(SOURCE_SKIPPED_EVENT, stats);
        }
        return !(stats.size > bytes(getOption('maxSize', this._options)));
      })
      .filter((stats: any) => {
        const { path } = stats;
        const source: string = sourceToString({ id: path } as ISourceOptions);
        const lines = source.split('\n').length;
        if (lines < getOption('minLines', this._options) || lines > getOption('maxLines', this._options)) {
          this.eventEmitter.emit(SOURCE_SKIPPED_EVENT, { ...stats, lines });
          return false;
        }
        return true;
      })
      .map((stats: any) => {
        const { path } = stats;
        const format: string = getFormatByFile(path, this._options.formatsExts) as string;
        const source: string = sourceToString({ id: path } as ISourceOptions);
        const sourceOptions: ISourceOptions = {
          id: path,
          format,
          detectionDate: new Date().getTime(),
          lastUpdateDate: stats.mtimeMs
        };
        return { source, sourceOptions };
      });

    for (const source of sources) {
      this._clones.push(...(await this._detect(source.source, source.sourceOptions)));
    }

    await this._detectionFinished(this._clones);
    return this._clones;
  }

  public on(event: string, fn: EventEmitter.ListenerFn, context?: any) {
    this.eventEmitter.on(event, fn, context);
  }

  private async _detect(source: string, options: ISourceOptions): Promise<IClone[]> {
    initLanguages([options.format]);
    const mode = getModeHandler(getOption('mode', this._options));
    const tokens: IToken[] = tokenize(source, options.format).filter(token => mode(token, this._options));

    const tokenMaps: TokensMap[] = createTokensMaps(tokens, getOption('minTokens', this._options)).map(tokenMap => {
      const subSource: ISourceOptions = {
        ...options,
        format: tokenMap.getFormat(),
        range: [tokenMap.getStartPosition(), tokenMap.getEndPosition()],
        lines: getSourceFragmentLength(options, tokenMap.getStartPosition(), tokenMap.getEndPosition())
      };
      tokenMap.setSourceId(options.id);
      this.eventEmitter.emit(MATCH_SOURCE_EVENT, subSource);
      return tokenMap;
    });

    return await Promise.all(
      tokenMaps.map(async (tokenMap: TokensMap) => {
        return await this.detector.detectByMap(tokenMap);
      })
    ).then((clns: any[]) => {
      return [].concat(...clns);
    });
  }

  private initializeReporters() {
    registerReportersByName(this._options);

    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.attach(this.eventEmitter);
    });
  }

  private initializeListeners() {
    registerListenerByName(this._options);

    Object.values(getRegisteredListeners()).map((listener: IListener) => {
      listener.attach(this.eventEmitter);
    });
  }

  private async _detectionFinished(clones: IClone[]) {
    await Promise.all(this._postHooks.map((hook: IHook) => hook.use(this)));
    await this.generateReports(clones);
    this.eventEmitter.emit(END_EVENT, clones);
  }

  private async generateReports(clones: IClone[]) {
    const statistic: IStatistic = await StoresManager.getStore(STATISTIC_DB).get(
      getOption('executionId', this.options)
    );
    Object.values(getRegisteredReporters()).map((reporter: IReporter) => {
      reporter.report(clones, statistic);
    });
  }
}
