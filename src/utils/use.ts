import { IMode } from '../interfaces/mode.type';

const detectInstalled = require('detect-installed');

export function useReporter(name: string) {
  const reporterName = `jscpd-${name}-reporter`;
  if (!detectInstalled.sync(reporterName, { local: true })) {
    throw new Error(`Reporter "${reporterName}" does not found, please check that you have installed reporter package`);
  }
  return require(reporterName).default;
}

export function useMode(name: string): IMode {
  const modeName = `jscpd-${name}-mode`;
  if (!detectInstalled.sync(modeName, { local: true })) {
    throw new Error(`Mode "${modeName}" does not found, please check that you have installed mode package`);
  }
  return require(modeName).default;
}
