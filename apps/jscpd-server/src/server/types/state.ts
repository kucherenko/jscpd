import { IStatistic } from '@jscpd/core';

export interface ServerState {
  workingDirectory: string;
  statistics: IStatistic | null;
  isScanning: boolean;
  lastScanTime: string | null;
}
