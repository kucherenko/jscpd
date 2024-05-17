import {getOption, IClone, ICloneValidator, IOptions, IValidationResult} from '@jscpd/core';
import {isAbsolute, relative} from "path";

export class SkipIsolatedValidator implements ICloneValidator {
  isRelativeMemoMap = new Map();


  validate(clone: IClone, options: IOptions): IValidationResult {
    const status = !this.shouldSkipClone(clone, options);
    return {
      status,
      clone,
      message: [
        `Sources of duplication located in isolated folder (${clone.duplicationA.sourceId}, ${clone.duplicationB.sourceId})`
      ]
    };
  }

  public shouldSkipClone(clone: IClone, options: IOptions): boolean {
    const skipIsolatedPathList: string[][] = getOption('skipIsolated', options);
    return skipIsolatedPathList.some(
      (dirList) => {
        const relA = dirList.find(dir => this.isRelativeMemo(clone.duplicationA.sourceId, dir));
        if (!relA) {
          return false;
        }
        const relB = dirList.find(dir => this.isRelativeMemo(clone.duplicationB.sourceId, dir));
        return relB && relA !== relB;
      }
    );
  }

  private isRelativeMemo(file: string, dir: string) {
    const memoKey = `${file},${dir}`;
    if (this.isRelativeMemoMap.has(memoKey)) return this.isRelativeMemoMap.get(memoKey);
    const isRel = SkipIsolatedValidator.isRelative(file, dir)
    this.isRelativeMemoMap.set(memoKey, isRel);
    return isRel;
  }

  private static isRelative(file: string, path: string): boolean {
    const rel = relative(path, file);
    return rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);
  }

}
