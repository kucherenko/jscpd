import {
  IClone,
  IMapFrame,
  IOptions,
  IStore,
  Statistic,
} from '@jscpd/core';
import { InFilesDetector } from '@jscpd/finder';
import { getSupportedFormats, Tokenizer, getFormatByFile } from '@jscpd/tokenizer';
import { createHash } from 'crypto';
import { createDetectorContext } from '../detect';
import {
  CheckSnippetRequest,
  CheckSnippetResponse,
  SnippetDuplication,
  ServerState,
} from './types';
import { writeFileSync, mkdirSync, existsSync, unlinkSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

const LANGUAGE_EXTENSIONS: Record<string, string> = {
  javascript: '.js',
  typescript: '.ts',
  python: '.py',
  java: '.java',
  csharp: '.cs',
  cpp: '.cpp',
  c: '.c',
  php: '.php',
  ruby: '.rb',
  go: '.go',
  rust: '.rs',
  swift: '.swift',
  kotlin: '.kt',
  scala: '.scala',
};

const SNIPPET_TEMP_DIR = 'jscpd-server-snippets';

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
      const context = createDetectorContext({
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

  private createSnippetFile(code: string, filename?: string, language?: string): { path: string; format: string | null } {
    const snippetDir = join(tmpdir(), SNIPPET_TEMP_DIR);
    if (!existsSync(snippetDir)) {
      mkdirSync(snippetDir, { recursive: true });
    }

    const snippetId = createHash('md5')
      .update(code + Date.now())
      .digest('hex');

    let finalFilename = filename || `snippet_${snippetId}`;

    if (language && !finalFilename.includes('.')) {
      finalFilename += LANGUAGE_EXTENSIONS[language.toLowerCase()] || '.txt';
    }

    const snippetPath = join(snippetDir, finalFilename);
    writeFileSync(snippetPath, code, 'utf-8');

    const format = language || getFormatByFile(snippetPath, this.options!.formatsExts);

    return { path: snippetPath, format };
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
    const percentageDuplicated = totalLines > 0
      ? Math.round((10000 * duplicatedLines) / totalLines) / 100
      : 0;

    return {
      totalDuplications: duplications.length,
      duplicatedLines,
      totalLines,
      percentageDuplicated,
    };
  }

  private cleanupSnippetFile(snippetPath: string): void {
    try {
      if (existsSync(snippetPath)) {
        unlinkSync(snippetPath);
      }
    } catch (error) {
      console.error('Failed to cleanup temporary file:', error);
    }
  }

  async checkSnippet(request: CheckSnippetRequest): Promise<CheckSnippetResponse> {
    if (!this.store || !this.options || !this.tokenizer || !this.statistic) {
      throw new Error('Server not initialized. Please wait for initial scan to complete.');
    }

    if (!request.code || request.code.trim().length === 0) {
      throw new Error('Code snippet cannot be empty');
    }

    const { path: snippetPath, format } = this.createSnippetFile(
      request.code,
      request.filename,
      request.language
    );

    try {
      if (!format) {
        throw new Error(
          'Unable to determine format for snippet. Please provide a valid language or filename with extension.'
        );
      }

      const snippetStatistic = new Statistic();
      const detector = new InFilesDetector(
        this.tokenizer,
        this.store,
        snippetStatistic,
        { ...this.options, silent: true }
      );

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
    } finally {
      this.cleanupSnippetFile(snippetPath);
    }
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
