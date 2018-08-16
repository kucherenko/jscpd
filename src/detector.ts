import {CLONE_EVENT, Events, MATCH_FILE_EVENT} from './events';
import {IClone} from './interfaces/clone.interface';
import {IMapFrame} from './interfaces/map-frame.interface';
import {IOptions} from './interfaces/options.interface';
import {ISource} from './interfaces/source.interface';
import {IStore} from './interfaces/store/store.interface';
import {IToken} from './interfaces/token/token.interface';
import {getModeByName} from './modes';
import {StoresManager} from './stores/stores-manager';
import {TokensMap} from './token-map';
import {groupByFormat, tokenize} from './tokenizer/';
import {generateHashForSource} from './utils';
import {CLONES_DB, getHashDbName, SOURCES_DB} from "./stores/models";
import {addClone, createClone, isCloneLinesBiggerLimit} from "./clone";

export class Detector {

  constructor(private options: IOptions) {
  }

  public detect(source: ISource): IClone[] {
    const sourceId: string = generateHashForSource(source);
    const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);

    const cachedClones: IClone[] | undefined = this.getStoredClones(source);

    if (cachedClones) {
      return cachedClones;
    }

    const tokens: IToken[] = tokenize(source.source, source.format)
      .filter(this.getModeHandler())
      .map(t => ({...t, sourceId}));

    const tokenMaps: TokensMap[] = this.generateMapsForFormats(tokens);

    source.lines = source.source.split("\n").length;
    sourcesStore.set(sourceId, source);

    let clones: IClone[] = [];

    tokenMaps.forEach(
      tokenMap => {
        Events.emit(MATCH_FILE_EVENT, {
          path: source.id,
          format: tokenMap.getFormat(),
          linesCount: tokenMap.getLinesCount()
        });
        source.formats = source.formats || {};
        source.formats[tokenMap.getFormat()] = tokenMap.getLinesCount();
        clones = clones.concat(...this.detectByMap(tokenMap));
      }
    );

    sourcesStore.set(sourceId, source);
    return clones.map(clone => ({...clone, is_new: true}));
  }

  private getStoredClones(source: ISource): IClone[] | undefined {
    const sourceId: string = generateHashForSource(source);
    const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);

    const sourceExist: ISource = sourcesStore.get(sourceId);

    if (
      this.options.cache &&
      sourceExist &&
      sourceExist.last_update === source.last_update
    ) {
      Object.entries(sourceExist.formats || {}).map(([format, lines]) => {
        Events.emit(MATCH_FILE_EVENT, {
          path: source.id,
          format: format,
          linesCount: lines
        });
      });
      const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
      return Object
        .values(clonesStore.getAll())
        .filter((clone: IClone) => {
          return clone.duplicationA.sourceId === sourceId;
        })
        .map((clone: IClone) => {
          clone.is_new = false;
          Events.emit(CLONE_EVENT, clone);
          return clone;
        });
    } else if (sourceExist) {
      this.removeSourcesArtifacts(sourceExist);
    }
    return undefined;
  }

  private removeSourcesArtifacts(source: ISource) {
    const clonesStore: IStore<ISource> = StoresManager.getStore(CLONES_DB);
    (source.clones || []).map((clone: string) => {
      clonesStore.delete(clone);
    });
    Object.entries(source.hashes || {}).map(([format, hashes]) => {
      const hashesStore = StoresManager.getStore(getHashDbName(format));
      hashes.map(hash => hashesStore.delete(hash));
    });
  }

  private detectByMap(tokenMap: TokensMap): IClone[] {
    const clones: IClone[] = [];
    if (tokenMap.getLength() >= this.options.minTokens) {
      let isClone: boolean = false;
      let start: IMapFrame | undefined;
      let end: IMapFrame | undefined;

      const HashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(tokenMap.getFormat()));

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
            const clone: IClone = createClone(
              start,
              end,
              tokenMap.getFormat()
            );
            if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
              clones.push(clone);
              addClone(clone);
            }
          }
          isClone = false;
          start = undefined;
          HashesStore.set(mapFrame.id, mapFrame);
          this.addHashToSource(mapFrame.id, tokenMap.getSourceId(), tokenMap.getFormat());
        }
      }

      if (isClone && start && end) {
        const clone: IClone = createClone(
          start,
          end,
          tokenMap.getFormat()
        );
        if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
          clones.push(clone);
          addClone(clone);
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

  private getModeHandler(): (token: IToken) => boolean {
    return typeof this.options.mode === 'string'
      ? getModeByName(this.options.mode)
      : this.options.mode;
  }

  private addHashToSource(hash: string, sourceId: string, format: string) {
    const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
    const source: ISource = sourcesStore.get(sourceId);
    if (source && source.hashes) {
      if (source.hashes.hasOwnProperty(format)) {
        source.hashes[format] = source.hashes[format].concat(hash);
      } else {
        source.hashes[format] = [hash]
      }
      sourcesStore.set(sourceId, source);
    }
  }
}
