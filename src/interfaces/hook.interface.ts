import { JSCPD } from '../jscpd';

export interface IHook {
  use(jscpd: JSCPD): Promise<any>;
}
