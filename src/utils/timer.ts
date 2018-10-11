const t = require('exectimer');

const TICKS: { [name: string]: any } = {};

export function timerStart(name: string) {
  if (!TICKS.hasOwnProperty(name)) {
    TICKS[name] = new t.Tick(name);
  }
  TICKS[name].start();
}
export function timerStop(name: string) {
  if (!TICKS.hasOwnProperty(name)) {
    throw new Error(`Timer ${name} not started yet`);
  }
  TICKS[name].stop();
}
