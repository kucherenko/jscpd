import {IClone, ICloneValidator, IMapFrame, IOptions, IStore, ITokenLocation, IValidationResult} from './interfaces';
import {runCloneValidators} from './validators';
import {ITokensMap} from '.';
import EventEmitter from "eventemitter3";

export class RabinKarp {
  constructor(
    private readonly options: IOptions,
    private readonly eventEmitter: EventEmitter,
    private readonly cloneValidators: ICloneValidator[],
  ) {
  }

  public async run(tokenMap: ITokensMap, store: IStore<IMapFrame>): Promise<IClone[]> {
    return new Promise((resolve => {
      let mapFrameInStore: any;
      let clone: IClone | null = null;

      const clones: IClone[] = [];

      // eslint-disable-next-line @typescript-eslint/explicit-function-return-type
      const loop = () => {
        const iteration = tokenMap.next();

				store
					.get(iteration.value.id)
					.then(
						(mapFrameFromStore: IMapFrame) => {
							mapFrameInStore = mapFrameFromStore;
							if (!clone) {
                clone = RabinKarp.createClone(tokenMap.getFormat(), iteration.value, mapFrameInStore);
              }
						},
						() => {
							if (clone && this.validate(clone)) {
								clones.push(clone);
							}
							clone = null;
							if (iteration.value.id) {
								return store.set(iteration.value.id, iteration.value);
							}
						},
          )
          .finally(() => {
            if (!iteration.done) {
              if (clone) {
                // @ts-ignore
                clone = RabinKarp.enlargeClone(clone, iteration.value, mapFrameInStore);
              }
              loop();
            } else {
              resolve(clones);
            }
          });
      }
      loop();
    }));
  }

  private validate(clone: IClone): boolean {

    const validation: IValidationResult = runCloneValidators(clone, this.options, this.cloneValidators);

    if (validation.status) {
      this.eventEmitter.emit('CLONE_FOUND', {clone})
    } else {
      this.eventEmitter.emit('CLONE_SKIPPED', {clone, validation})
    }
    return validation.status;
  }

  private static createClone(format: string, mapFrameA: IMapFrame, mapFrameB: IMapFrame): IClone {
    return {
      format,
      foundDate: new Date().getTime(),
      duplicationA: {
        sourceId: mapFrameA.sourceId,
        start: mapFrameA?.start?.loc?.start as ITokenLocation,
        end: mapFrameA?.end?.loc?.end as ITokenLocation,
        range: [mapFrameA.start.range[0], mapFrameA.end.range[1]],
      },
      duplicationB: {
        sourceId: mapFrameB.sourceId,
        start: mapFrameB?.start?.loc?.start as ITokenLocation,
        end: mapFrameB?.end?.loc?.end as ITokenLocation,
        range: [mapFrameB.start.range[0], mapFrameB.end.range[1]],
      },
    }
  }

  private static enlargeClone(clone: IClone, mapFrameA: IMapFrame, mapFrameB: IMapFrame): IClone {
    clone.duplicationA.range[1] = mapFrameA.end.range[1];
    clone.duplicationA.end = mapFrameA?.end?.loc?.end as ITokenLocation;
    clone.duplicationB.range[1] = mapFrameB.end.range[1];
    clone.duplicationB.end = mapFrameB?.end?.loc?.end as ITokenLocation;
    return clone;
  }

}


