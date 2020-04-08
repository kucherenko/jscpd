import { grey } from 'colors/safe';
import { isAbsolute, relative } from 'path';
import { IClone, IOptions } from '..';
import { ISkiper } from '../interfaces/skiper.interface';
import { getOption } from '../utils/options';

export class LocalSkiper implements ISkiper {
  public shouldSkipClone(clone: IClone, options: IOptions): boolean {
    const shouldSkipLocal: boolean = getOption('skipLocal', options);
    const path: string[] = getOption('path', options);
    if (path.length < 2 && shouldSkipLocal) {
      console.log(grey("Warning: --skipLocal options works if provided more then two path's"));
      console.log(grey('Example: jscpd --skipLocal /first/path /second/path'));
      console.log('');
      return false;
    }
    return path.some(
      (dir) => this.isRelative(clone.duplicationA.sourceId, dir) && this.isRelative(clone.duplicationB.sourceId, dir)
    );
  }

  private isRelative(file: string, path: string): boolean {
    const rel = relative(path, file);
    return rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);
  }
}
