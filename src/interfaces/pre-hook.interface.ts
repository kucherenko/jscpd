import { JSCPD } from '../jscpd';

export interface IPreHook {
  use(jscpd: JSCPD): Promise<any>;
}
