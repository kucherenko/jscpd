import { createClone, getSourceFragmentLength, isCloneLinesBiggerLimit } from './clone';
import { CLONE_EVENT, HASH_EVENT, MATCH_SOURCE_EVENT } from './events';
import { IClone } from './interfaces/clone.interface';
import { IMapFrame } from './interfaces/map-frame.interface';
import { IOptions } from './interfaces/options.interface';
import { ISource } from './interfaces/source.interface';
import { IStore } from './interfaces/store/store.interface';
import { IToken } from './interfaces/token/token.interface';
import { JSCPD } from './jscpd';
import { getModeHandler } from './modes';
import { getHashDbName } from './stores/models';
import { StoresManager } from './stores/stores-manager';
import { TokensMap } from './token-map';
import { groupByFormat, tokenize } from './tokenizer/';
import { generateSourceId } from './utils';

export class Detector {
  constructor(private options: IOptions) {}

  public detect(source: ISource) {
    const tokens: IToken[] = tokenize(source.source, source.format).filter(getModeHandler(this.options));

    const tokenMaps: TokensMap[] = this.generateMapsForFormats(tokens);

    tokenMaps.forEach(tokenMap => {
      const subSource: ISource = {
        ...source,
        format: tokenMap.getFormat(),
        range: [tokenMap.getStartPosition(), tokenMap.getEndPosition()],
        lines: getSourceFragmentLength(source, tokenMap.getStartPosition(), tokenMap.getEndPosition())
      };
      tokenMap.setSourceId(generateSourceId(subSource));
      JSCPD.getEventsEmitter().emit(MATCH_SOURCE_EVENT, subSource);
      this.detectByMap(tokenMap);
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
            const clone: IClone = createClone(start, end);
            if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
              clones.push(clone);
              JSCPD.getEventsEmitter().emit(CLONE_EVENT, clone);
            }
          }
          isClone = false;
          start = undefined;
          HashesStore.set(mapFrame.id, mapFrame);
          JSCPD.getEventsEmitter().emit(HASH_EVENT, mapFrame);
        }
      }

      if (isClone && start && end) {
        const clone: IClone = createClone(start, end);
        if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
          clones.push(clone);
          JSCPD.getEventsEmitter().emit(CLONE_EVENT, clone);
        }
      }
    }
    return clones;
  }

  private generateMapsForFormats(tokens: IToken[]): TokensMap[] {
    return Object.values(groupByFormat(tokens)).map(toks => new TokensMap(toks, toks[0].format, this.options));
  }
}
