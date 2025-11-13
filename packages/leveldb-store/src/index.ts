import {IMapFrame, IStore} from '@jscpd-ai/core';
import {ensureDirSync} from 'fs-extra';
import {sync} from "rimraf";
import {Level} from "level";


export default class LevelDbStore implements IStore<IMapFrame> {
  private name: string = '';
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private dbs: Record<string, any> = {};

  get(key: string): Promise<IMapFrame> {
    return this.dbs[this.name].get(key).then((value: string) => JSON.parse(value));
  }

  namespace(name: string): void {
    this.name = name;
    if (!(name in this.dbs)) {
      const path = `.jscpd/${name}`;
      sync(path);
      ensureDirSync(path);
      this.dbs[name] = new Level(path);
    }
  }

  set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return this.dbs[this.name].put(key, JSON.stringify(value));
  }

  close(): void {
    Object.entries(this.dbs).forEach(([name, db]) => {
      db.close(() => {
        try {
          sync('.jscpd/' + name)
        } catch (e) {
          console.log(e);
        }
      });
    });
    sync('.jscpd');
  }
}
