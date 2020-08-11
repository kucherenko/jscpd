import {getOption, IOptions} from '@jscpd/core';
import {Entry, sync} from 'fast-glob';
import {getFormatByFile} from '@jscpd/tokenizer';
import {readFileSync} from 'fs-extra';
import {grey} from 'colors/safe';
import {EntryWithContent} from './interfaces';
import {lstatSync, Stats} from "fs";
import bytes = require('bytes');

function isFile(path: string): boolean {
  try {
    const stat: Stats = lstatSync(path);
    return stat.isFile();
  } catch (e) {
    // lstatSync throws an error if path doesn't exist
    return false;
  }
}

function skipNotSupportedFormats(options: IOptions): (entry: Entry) => boolean {
  return (entry: Entry): boolean => {
    const {path} = entry;
    const format: string = getFormatByFile(path, options.formatsExts);
    const shouldNotSkip = format && options.format && options.format.includes(format);
    if ((options.debug || options.verbose) && !shouldNotSkip) {
      console.log(`File ${path} skipped! Format "${format}" does not included to supported formats.`);
    }
    return shouldNotSkip;
  }
}

function skipBigFiles(options: IOptions): (entry: Entry) => boolean {
  return (entry: Entry): boolean => {
    const {stats, path} = entry;
    const shouldSkip = bytes.parse(stats.size) > bytes.parse(getOption('maxSize', options));
    if (options.debug && shouldSkip) {
      console.log(`File ${path} skipped! Size more then limit (${bytes(stats.size)} > ${getOption('maxSize', options)})`);
    }
    return !shouldSkip;
  };
}

function skipFilesIfLinesOfContentNotInLimits(options: IOptions): (entry: EntryWithContent) => boolean {
  return (entry: EntryWithContent): boolean => {
    const {path, content} = entry;
    const lines = content.split('\n').length;
    const minLines = getOption('minLines', options);
    const maxLines = getOption('maxLines', options);
    if (lines < minLines || lines > maxLines) {
      if ((options.debug || options.verbose)) {
        console.log(grey(`File ${path} skipped! Code lines=${lines} not in limits (${minLines}:${maxLines})`));
      }
      return false;
    }
    return true;
  }
}

function addContentToEntry(entry: Entry): EntryWithContent {
  const {path} = entry;
  const content = readFileSync(path).toString();
  return {...entry, content}
}

export function getFilesToDetect(options: IOptions): EntryWithContent[] {
  const pattern = options.pattern || '**/*';
  const patterns = options.path.map((path: string) => {
    if (isFile(path)) {
      return path;
    }
    return path.substr(path.length - 1) === '/' ? `${path}${pattern}` : `${path}/${pattern}`;
  });
  return sync(
    patterns,
    {
      ignore: options.ignore,
      onlyFiles: true,
      dot: true,
      stats: true,
      absolute: options.absolute,
      followSymbolicLinks: !options.noSymlinks,
    },
  )
    .filter(skipNotSupportedFormats(options))
    .filter(skipBigFiles(options))
    .map(addContentToEntry)
    .filter(skipFilesIfLinesOfContentNotInLimits(options));
}

