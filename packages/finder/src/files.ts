import {getOption, IOptions} from '@jscpd/core';
import {sync} from 'fast-glob';
import {getFormatByFile} from '@jscpd/tokenizer';
import { readFileSync, realpathSync } from 'fs-extra';
import {grey} from 'colors/safe';
import {EntryWithContent, IEntry} from './interfaces';
import {lstatSync, existsSync, Stats} from "fs";
import bytes from "bytes";
import * as path from 'path';

const SHEBANG_TO_FORMAT: Record<string, string> = {
  bash: 'bash',
  sh: 'bash',
  zsh: 'bash',
  dash: 'bash',
  ksh: 'bash',
  python: 'python',
  ruby: 'ruby',
  perl: 'perl',
  php: 'php',
  node: 'javascript',
  nodejs: 'javascript',
  lua: 'lua',
  tclsh: 'tcl',
  wish: 'tcl',
  groovy: 'groovy',
  awk: 'awk',
  gawk: 'awk',
  nawk: 'awk',
  rscript: 'r',
};

function readShebangFormat(filePath: string): string | undefined {
  try {
    const buf = readFileSync(filePath, {encoding: null} as any) as unknown as Buffer;
    const head = buf.slice(0, 128).toString('utf8');
    const firstLine = head.split('\n')[0] ?? '';
    if (!firstLine.startsWith('#!')) {
      return undefined;
    }
    const line = firstLine.slice(2).trim();
    const tokens = line.split(/\s+/).filter(Boolean);
    if (tokens.length === 0) {
      return undefined;
    }
    const firstToken = tokens[0] as string;
    let interpreterToken: string;
    if (path.basename(firstToken).startsWith('env')) {
      const secondToken = tokens[1];
      if (!secondToken || secondToken.startsWith('-')) {
        return undefined;
      }
      interpreterToken = secondToken;
    } else {
      interpreterToken = firstToken;
    }
    const rawName = path.basename(interpreterToken);
    if (/^\d/.test(rawName)) {
      return undefined;
    }
    const normalized = rawName.replace(/[0-9]+(\.[0-9]+)*$/, '').toLowerCase();
    if (normalized === '') {
      return undefined;
    }
    return SHEBANG_TO_FORMAT[normalized];
  } catch {
    return undefined;
  }
}


function isFile(path: string): boolean {
  try {
    const stat: Stats = lstatSync(path);
    return stat.isFile();
  } catch (e) {
    // lstatSync throws an error if path doesn't exist
    return false;
  }
}

function isSymlink(path: string): boolean {
  try {
    const stat: Stats = lstatSync(path);
    return stat.isSymbolicLink();
  } catch (e) {
    // lstatSync throws an error if path doesn't exist
    return false;
  }
}

function skipNotSupportedFormats(options: IOptions): (entry: IEntry) => boolean {
  return (entry: IEntry): boolean => {
    const {path} = entry;
    let format: string | undefined = getFormatByFile(path, options.formatsExts, options.formatsNames);

    if (format === undefined && entry.stats?.mode !== undefined && (entry.stats.mode & 0o111) !== 0) {
      if (!isSymlink(path)) {
        format = readShebangFormat(path);
        entry.detectedFormat = format;
      }
    }

    const shouldNotSkip = !!(format && options.format && options.format.includes(format));
    if ((options.debug || options.verbose) && !shouldNotSkip) {
      console.log(`File ${path} skipped! Format "${format}" does not included to supported formats.`);
    }
    return shouldNotSkip;
  }
}

function skipBigFiles(options: IOptions): (entry: IEntry) => boolean {
  return (entry: IEntry): boolean => {
    const {stats, path} = entry;
    if (!stats) {
      return true;
    }
    // @ts-expect-error - stats is checked above, but DTS build doesn't recognize control flow
    const shouldSkip = bytes.parse(stats.size) > bytes.parse(getOption('maxSize', options) || '0');
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

function addContentToEntry(entry: IEntry): EntryWithContent {
  const {path} = entry;
  const content = readFileSync(path).toString();
  return {...entry, content}
}

function gitignoreLineToGlobs(line: string, baseDir: string): string[] {
  const trimmed = line.trim();

  if (!trimmed || trimmed.startsWith('#')) return [];

  if (trimmed.startsWith('!')) {
    const inner = gitignoreLineToGlobs(trimmed.slice(1), baseDir);
    return inner.map(p => `!${p}`);
  }

  let pattern = trimmed;

  const isRooted = pattern.startsWith('/');
  if (isRooted) pattern = pattern.slice(1);

  if (pattern.endsWith('/')) pattern = pattern.slice(0, -1);

  const hasMiddleSlash = pattern.includes('/');
  const normalizedBase = baseDir.replace(/\\/g, '/');

  if (isRooted || hasMiddleSlash) {
    const glob = `${normalizedBase}/${pattern}`;
    return [glob, `${glob}/**`];
  }

  return [`**/${pattern}`, `**/${pattern}/**`];
}

function collectGitignorePatterns(dirs: string[]): string[] {
  const patterns: string[] = [];
  const visited = new Set<string>();

  for (const dir of dirs) {
    const absDir = path.resolve(dir);
    if (visited.has(absDir)) continue;
    visited.add(absDir);

    const gitignorePath = path.join(absDir, '.gitignore');
    if (!existsSync(gitignorePath)) continue;

    try {
      const content = readFileSync(gitignorePath, 'utf8');
      for (const line of content.split('\n')) {
        patterns.push(...gitignoreLineToGlobs(line, absDir));
      }
    } catch {
      // unreadable .gitignore — skip silently
    }
  }

  return patterns;
}

export function getFilesToDetect(options: IOptions): EntryWithContent[] {
  const pattern = options.pattern || '**/*';
  let patterns = options.path;
  const cwd = process.cwd();

  if (options.noSymlinks) {
    patterns = patterns!==undefined ? patterns.filter((path: string) => !isSymlink(path)) : [];
  }

  // Capture scan directories before appending the glob pattern, so we can resolve
  // ignore patterns relative to each scan directory below.
  const scanDirs: string[] = (patterns || []).map((inputPath: string) => {
    try {
      return isFile(realpathSync(inputPath)) ? path.dirname(inputPath) : inputPath;
    } catch {
      return inputPath;
    }
  });

  patterns = patterns!==undefined ? patterns.map((inputPath: string) => {
    const currentPath = realpathSync(inputPath);

    if (isFile(currentPath)) {
      return inputPath;
    }

    return inputPath.endsWith('/') ? `${inputPath}${pattern}` : `${inputPath}/${pattern}`;
  }): [];

  // Normalize ignore patterns so they work regardless of whether the scan path
  // is relative or absolute and regardless of whether it equals cwd (issue #611).
  //
  // fast-glob returns relative result paths for relative scan patterns and
  // absolute result paths for absolute patterns. A pattern like "./ada/**" won't
  // match either "fixtures/ada/file.js" (relative, when scanning "./fixtures")
  // or "/cwd/fixtures/ada/file.js" (absolute, when scanning an absolute path).
  //
  // For each relative ignore pattern we generate additional variants:
  //   • original                        – keeps backward-compat for trivial cases
  //   • path.join(scanDir, pattern)     – matches relative result paths
  //   • path.resolve(cwd, scanDir, pattern) – matches absolute result paths
  // Patterns already starting with "**/" already work and are left unchanged.
  const normalizedIgnore = (options.ignore || []).flatMap((ignorePattern: string) => {
    if (path.isAbsolute(ignorePattern) || ignorePattern.startsWith('**/')) {
      return [ignorePattern];
    }
    const variants = new Set<string>([ignorePattern]);
    for (const scanDir of [...scanDirs, '.']) {
      variants.add(path.join(scanDir, ignorePattern));
      variants.add(path.resolve(cwd, scanDir, ignorePattern));
    }
    return [...variants];
  });

  if (options.gitignore) {
    normalizedIgnore.push(...collectGitignorePatterns(scanDirs));
  }

  return (sync(
    patterns,
    {
      ignore: normalizedIgnore,
      onlyFiles: true,
      dot: true,
      stats: true,
      absolute: options.absolute || false,
      followSymbolicLinks: !options.noSymlinks,
      cwd: process.cwd(),
    },
  ) as IEntry[])
    .filter(skipNotSupportedFormats(options))
    .filter(skipBigFiles(options))
    .map(addContentToEntry)
    .filter(skipFilesIfLinesOfContentNotInLimits(options));
}

