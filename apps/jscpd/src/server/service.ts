import {
  IClone,
  IMapFrame,
  IOptions,
  IStore,
  Statistic,
} from '@jscpd/core';
import { InFilesDetector } from '@jscpd/finder';
import { FORMATS, getSupportedFormats, Tokenizer } from '@jscpd/tokenizer';
import { createBaseDetectorContext } from '../detect';
import {
  CheckSnippetRequest,
  CheckSnippetResponse,
  SnippetDuplication,
  ServerState,
} from './types';

function calculatePercentage(total: number, cloned: number): number {
  return total ? Math.round((10000 * cloned) / total) / 100 : 0.0;
}

function getExtensionForLanguage(language: string): string | undefined {
  const normalizedLang = language.toLowerCase();
  const format = Object.keys(FORMATS).find((key) => key === normalizedLang);
  return format && FORMATS[format].exts.length > 0 ? FORMATS[format].exts[0] : undefined;
}

export class JscpdServerService {
  private state: ServerState;
  private store: IStore<IMapFrame> | null = null;
  private options: IOptions | null = null;
  private statistic: Statistic | null = null;
  private tokenizer: Tokenizer | null = null;

  constructor(workingDirectory: string) {
    this.state = {
      workingDirectory,
      statistics: null,
      isScanning: false,
      lastScanTime: null,
    };
  }

  async initialize(options: Partial<IOptions> = {}): Promise<void> {
    if (this.state.isScanning) {
      throw new Error('Scan already in progress');
    }

    this.state.isScanning = true;

    try {
      const context = createBaseDetectorContext({
        ...options,
        path: [this.state.workingDirectory],
        format: options.format || getSupportedFormats(),
        silent: true,
      });

      this.options = context.options;
      this.store = context.store;
      this.statistic = context.statistic;
      this.tokenizer = context.tokenizer;

      await context.detector.detect(context.files);

      this.state.statistics = this.statistic.getStatistic();
      this.state.lastScanTime = new Date().toISOString();
    } finally {
      this.state.isScanning = false;
    }
  }

  private getSnippetPath(filename?: string, language?: string): string {
    if (filename) {
      return `<snippet>/${filename}`;
    }

    const hashFunction = this.options?.hashFunction;
    const snippetId = hashFunction
      ? hashFunction(Date.now().toString()).slice(0, 8)
      : Date.now().toString().slice(-8);

    if (language) {
      const ext = getExtensionForLanguage(language);
      if (ext) {
        return `<snippet>/snippet_${snippetId}.${ext}`;
      }
    }

    return `<snippet>/snippet_${snippetId}`;
  }

  private filterSnippetClones(clones: IClone[], snippetPath: string): IClone[] {
    return clones.filter(
      (clone) =>
        clone.duplicationA.sourceId === snippetPath ||
        clone.duplicationB.sourceId === snippetPath
    );
  }

  private mapCloneToDuplication(clone: IClone, snippetPath: string): SnippetDuplication {
    const isSnippetInA = clone.duplicationA.sourceId === snippetPath;
    const snippetDup = isSnippetInA ? clone.duplicationA : clone.duplicationB;
    const codebaseDup = isSnippetInA ? clone.duplicationB : clone.duplicationA;

    return {
      snippetLocation: {
        startLine: snippetDup.start.line,
        endLine: snippetDup.end.line,
        startColumn: snippetDup.start.column,
        endColumn: snippetDup.end.column,
      },
      codebaseLocation: {
        file: codebaseDup.sourceId.replace(this.state.workingDirectory, '').replace(/^\//, ''),
        startLine: codebaseDup.start.line,
        endLine: codebaseDup.end.line,
        startColumn: codebaseDup.start.column,
        endColumn: codebaseDup.end.column,
        fragment: codebaseDup.fragment,
      },
      linesCount: snippetDup.end.line - snippetDup.start.line,
    };
  }

  private calculateDuplicationStatistics(
    duplications: SnippetDuplication[],
    totalLines: number
  ): CheckSnippetResponse['statistics'] {
    const duplicatedLinesSet = new Set<number>();

    duplications.forEach((dup) => {
      for (let i = dup.snippetLocation.startLine; i <= dup.snippetLocation.endLine; i++) {
        duplicatedLinesSet.add(i);
      }
    });

    const duplicatedLines = duplicatedLinesSet.size;
    const percentageDuplicated = calculatePercentage(totalLines, duplicatedLines);

    return {
      totalDuplications: duplications.length,
      duplicatedLines,
      totalLines,
      percentageDuplicated,
    };
  }

  private createSnippetDetector(): InFilesDetector {
    if (!this.store || !this.options || !this.tokenizer) {
      throw new Error('Server not initialized');
    }

    const snippetStatistic = new Statistic();
    return new InFilesDetector(
      this.tokenizer,
      this.store,
      snippetStatistic,
      { ...this.options, silent: true }
    );
  }

  async checkSnippet(request: CheckSnippetRequest): Promise<CheckSnippetResponse> {
    if (!this.store || !this.options || !this.tokenizer) {
      throw new Error('Server not initialized. Please wait for initial scan to complete.');
    }

    if (!request.code || request.code.trim().length === 0) {
      throw new Error('Code snippet cannot be empty');
    }

    const snippetPath = this.getSnippetPath(request.filename, request.language);
    const detector = this.createSnippetDetector();

    const clones = await detector.detect([
      { path: snippetPath, content: request.code },
    ]);

    const snippetClones = this.filterSnippetClones(clones, snippetPath);
    const duplications = snippetClones.map((clone) =>
      this.mapCloneToDuplication(clone, snippetPath)
    );

    const totalLines = request.code.split('\n').length;
    const statistics = this.calculateDuplicationStatistics(duplications, totalLines);

    return { duplications, statistics };
  }

  getStatistics() {
    return {
      statistics: this.state.statistics,
      timestamp: this.state.lastScanTime || new Date().toISOString(),
    };
  }

  getState(): ServerState {
    return { ...this.state };
  }

  async close(): Promise<void> {
    if (this.store) {
      await this.store.close();
    }
  }
}
