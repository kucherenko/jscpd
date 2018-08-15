import { Events } from './events';
import { IClone } from './interfaces/clone.interface';
import { IMapFrame } from './interfaces/map-frame.interface';
import { IOptions } from './interfaces/options.interface';
import { ISource } from './interfaces/source.interface';
import { IStore } from './interfaces/store/store.interface';
import { IToken } from './interfaces/token/token.interface';
import { getModeByName } from './modes';
import { StoresManager } from './stores/stores-manager';
import { TokensMap } from './token-map';
import { groupByFormat, tokenize } from './tokenizer/';
import { generateHashForSource, md5 } from './utils';

export class Detector {
  private static addClone(clone: IClone) {
    const clonesStore: IStore<IClone> = StoresManager.get('clones');
    clonesStore.set(md5(JSON.stringify(clone)), clone);
    Events.emit('clone', { ...clone, is_new: true });
  }

  private static createClone(
    startMap: IMapFrame,
    endMap: IMapFrame,
    format: string
  ): IClone {
    const hashesStore: IStore<IMapFrame> = StoresManager.get(
      'hashes.' + format
    );

    const sourceStart: IMapFrame = hashesStore.get(startMap.id);
    const sourceEnd: IMapFrame = hashesStore.get(endMap.id);

    const fragment: string = Detector.getFragment(
      startMap.start.sourceId,
      startMap.start.range[0],
      endMap.end.range[1]
    );

    const clone: IClone = {
      format,
      fragment,
      duplicationA: {
        sourceId: startMap.start.sourceId,
        start: startMap.start,
        end: endMap.end
      },
      duplicationB: {
        sourceId: sourceStart.start.sourceId,
        start: sourceStart.start,
        end: sourceEnd.end
      }
    };
    return clone;
  }

  private static getFragment(
    sourceId: string,
    start: number,
    end: number
  ): string {
    const sourcesStore: IStore<ISource> = StoresManager.get('source');
    return sourcesStore.get(sourceId).source.substring(start, end);
  }
  constructor(private options: IOptions) {}

  public detect(source: ISource): IClone[] {
    const sourceId: string = generateHashForSource(source);
    const sourcesStore: IStore<ISource> = StoresManager.get('source');

    const sourceExist: ISource = sourcesStore.get(sourceId);

    if (
      this.options.cache &&
      sourceExist &&
      sourceExist.last_update === source.last_update
    ) {
      const clonesStore: IStore<IClone> = StoresManager.get('clones');
      return Object.values(clonesStore.getAll())
        .filter((clone: IClone) => {
          return clone.duplicationA.sourceId === sourceId;
        })
        .map((clone: IClone) => {
          clone.is_new = false;
          Events.emit('clone', clone);
          return clone;
        });
    }

    sourcesStore.set(sourceId, source);

    const tokens: IToken[] = this.tokenize(source.source, source.format).map(
      t => ({ ...t, sourceId })
    );

    const tokenMaps: TokensMap[] = this.generateMapsForFormats(tokens);

    let clones: IClone[] = [];
    tokenMaps.forEach(
      tokenMap => (clones = clones.concat(...this.detectByMap(tokenMap)))
    );

    return clones.map(clone => ({ ...clone, is_new: true }));
  }

  private detectByMap(tokenMap: TokensMap): IClone[] {
    const clones: IClone[] = [];
    if (tokenMap.getLength() >= this.options.minTokens) {
      let isClone: boolean = false;
      let start: IMapFrame | undefined;
      let end: IMapFrame | undefined;
      const HashesStore: IStore<IMapFrame> = StoresManager.get(
        'hashes.' + tokenMap.getFormat()
      );
      for (const mapFrame of tokenMap) {
        if (HashesStore.has(mapFrame.id)) {
          isClone = true;
          if (!start) {
            start = end = mapFrame;
          } else {
            end = mapFrame;
          }
        } else {
          if (isClone && start && end) {
            const clone: IClone = Detector.createClone(
              start,
              end,
              tokenMap.getFormat()
            );
            if (this.isCloneLinesBiggerLimit(clone)) {
              clones.push(clone);
              Detector.addClone(clone);
            }
          }
          isClone = false;
          start = undefined;
          HashesStore.set(mapFrame.id, mapFrame);
        }
      }
      if (isClone && start && end) {
        const clone: IClone = Detector.createClone(
          start,
          end,
          tokenMap.getFormat()
        );
        if (this.isCloneLinesBiggerLimit(clone)) {
          clones.push(clone);
          Detector.addClone(clone);
        }
      }
    }
    return clones;
  }

  private generateMapsForFormats(tokens: IToken[]): TokensMap[] {
    return Object.values(groupByFormat(tokens)).map(
      toks => new TokensMap(toks, toks[0].format, this.options)
    );
  }

  private tokenize(code: string, format: string): IToken[] {
    return tokenize(code, format).filter(this.getModeHandler());
  }

  private getModeHandler(): (token: IToken) => boolean {
    return typeof this.options.mode === 'string'
      ? getModeByName(this.options.mode)
      : this.options.mode;
  }

  private isCloneLinesBiggerLimit(clone: IClone): boolean {
    return (
      clone.duplicationA.end.loc.end.line -
        clone.duplicationA.start.loc.end.line >=
      this.options.minLines
    );
  }
}
