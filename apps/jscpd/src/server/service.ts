import path from 'node:path';
import {
  Detector,
  IClone,
  ICloneValidator,
  IMapFrame,
  IOptions,
  IStore,
  MemoryStore,
  Statistic,
} from '@jscpd/core';
import { getSupportedFormats, Tokenizer } from '@jscpd/tokenizer';
import { createBaseDetectorContext } from '../detect';
import {
  CheckSnippetRequest,
  CheckSnippetResponse,
  SnippetDuplication,
  ServerState,
} from './types';
import { ERROR_MESSAGES } from './constants';
import { EphemeralHybridStore } from './ephemeral-store';

function calculatePercentage(total: number, cloned: number): number {
  return total ? Math.round((10000 * cloned) / total) / 100 : 0.0;
}

export class JscpdServerService {
  private state: ServerState;
  private store: IStore<IMapFrame> | null = null;
  private options: IOptions | null = null;
  private statistic: Statistic | null = null;
  private tokenizer: Tokenizer | null = null;
  private detector: Detector | null = null;
  private snippetCounter: number = 0;

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
      throw new Error(ERROR_MESSAGES.SCAN_IN_PROGRESS);
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

      const validators: ICloneValidator[] = [];
      this.detector = new Detector(this.tokenizer, this.store, validators, this.options);

      await context.detector.detect(context.files);

      this.state.statistics = this.statistic.getStatistic();
      this.state.lastScanTime = new Date().toISOString();
    } finally {
      this.state.isScanning = false;
    }
  }

  private generateSnippetId(): string {
    const hashFunction = this.options?.hashFunction;
    const base = `${Date.now()}_${this.snippetCounter++}`;
    const id = hashFunction ? hashFunction(base) : base;
    return `<snippet>/snippet_${id.slice(0, 8)}`;
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
        startColumn: snippetDup.start.column ?? 0,
        endColumn: snippetDup.end.column ?? 0,
      },
      codebaseLocation: {
        file: path.relative(this.state.workingDirectory, codebaseDup.sourceId),
        startLine: codebaseDup.start.line,
        endLine: codebaseDup.end.line,
        startColumn: codebaseDup.start.column ?? 0,
        endColumn: codebaseDup.end.column ?? 0,
        fragment: codebaseDup.fragment,
      },
      linesCount: snippetDup.end.line - snippetDup.start.line + 1,
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

  private async createEphemeralStore(format: string): Promise<IStore<IMapFrame>> {
    if (!this.store) {
      throw new Error(ERROR_MESSAGES.SOURCE_STORE_NOT_INITIALIZED);
    }

    const ephemeralMemoryStore = new MemoryStore<IMapFrame>();
    const hybridStore = new EphemeralHybridStore(this.store, ephemeralMemoryStore);
    hybridStore.namespace(format);

    return hybridStore;
  }

  async checkSnippet(request: CheckSnippetRequest): Promise<CheckSnippetResponse> {
    if (this.state.isScanning) {
      throw new Error(ERROR_MESSAGES.SCAN_IN_PROGRESS);
    }

    if (!this.store || !this.options || !this.tokenizer || !this.detector) {
      throw new Error(ERROR_MESSAGES.NOT_INITIALIZED);
    }

    if (!request.code || request.code.trim().length === 0) {
      throw new Error(ERROR_MESSAGES.EMPTY_CODE);
    }

    const snippetId = this.generateSnippetId();

    // Create ephemeral store seeded with project data to prevent snippet token
    // contamination in the shared store and ensure automatic cleanup after request
    const ephemeralStore = await this.createEphemeralStore(request.format);

    try {
      const validators: ICloneValidator[] = [];
      const ephemeralDetector = new Detector(
        this.tokenizer,
        ephemeralStore,
        validators,
        this.options
      );

      const clones = await ephemeralDetector.detect(snippetId, request.code, request.format);

      const snippetClones = this.filterSnippetClones(clones, snippetId);
      const duplications = snippetClones.map((clone) =>
        this.mapCloneToDuplication(clone, snippetId)
      );

      const totalLines = request.code.split('\n').length;
      const statistics = this.calculateDuplicationStatistics(duplications, totalLines);

      return { duplications, statistics };
    } finally {
      ephemeralStore.close();
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

    this.store = null;
    this.options = null;
    this.tokenizer = null;
    this.detector = null;
    this.snippetCounter = 0;
    this.state.statistics = null;
    this.state.lastScanTime = null;
  }
}
