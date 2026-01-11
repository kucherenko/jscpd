import { IStatistic } from '@jscpd/core';

export interface DuplicationLocation {
  file: string;
  startLine: number;
  endLine: number;
  startColumn: number;
  endColumn: number;
  fragment?: string;
}

export interface SnippetLocation {
  startLine: number;
  endLine: number;
  startColumn: number;
  endColumn: number;
}

export interface SnippetDuplication {
  snippetLocation: SnippetLocation;
  codebaseLocation: DuplicationLocation;
  linesCount: number;
}

export interface DuplicationStatistics {
  totalDuplications: number;
  duplicatedLines: number;
  totalLines: number;
  percentageDuplicated: number;
}

export interface CheckSnippetResponse {
  duplications: SnippetDuplication[];
  statistics: DuplicationStatistics;
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

export interface HealthResponse {
  status: 'initializing' | 'ready';
  workingDirectory: string;
  lastScanTime: string | null;
}

export interface ApiInfoResponse {
  name: string;
  version: string;
  endpoints: Record<string, string>;
  documentation: string;
}
