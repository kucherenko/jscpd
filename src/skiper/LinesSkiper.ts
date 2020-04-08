import { IClone, IOptions } from '..';
import { ISkiper } from '../interfaces/skiper.interface';
import { getOption } from '../utils/options';

export class LinesSkiper implements ISkiper {
  public shouldSkipClone(clone: IClone, options: IOptions): boolean {
    return clone.duplicationA.end.line - clone.duplicationA.start.line <= getOption('minLines', options);
  }
}
