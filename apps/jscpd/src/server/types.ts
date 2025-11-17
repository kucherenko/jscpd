import { IStatistic } from '@jscpd/core';

export interface CheckSnippetRequest {
  code: string;
  language?: string;
  filename?: string;
}

export interface DuplicationLocation {
  file: string;
  startLine: number;
  endLine: number;
  startColumn: number;
  endColumn: number;
  fragment?: string;
}

export interface SnippetDuplication {
  snippetLocation: {
    startLine: number;
    endLine: number;
    startColumn: number;
    endColumn: number;
  };
  codebaseLocation: DuplicationLocation;
  linesCount: number;
}

export interface CheckSnippetResponse {
  duplications: SnippetDuplication[];
  statistics: {
    totalDuplications: number;
    duplicatedLines: number;
    totalLines: number;
    percentageDuplicated: number;
  };
}

export interface ErrorResponse {
  error: string;
  message: string;
  statusCode: number;
}

export interface StatsResponse {
  statistics: IStatistic;
  timestamp: string;
}

export interface ServerState {
  workingDirectory: string;
  statistics: IStatistic | null;
  isScanning: boolean;
  lastScanTime: string | null;
}
