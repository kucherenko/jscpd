import {expect} from "chai";
import {compareDates, escapeXml} from "../src/utils/reports";

describe('jscpd finder: utils', () => {
  describe('escapeXml', () => {
    it('should replace unsafe symbols', () => {
      expect(escapeXml(`<>&'"`)).to.eq('&lt;&gt;&amp;&apos;&quot;')
    });
  });

  describe('compareDates', () => {
    it('should show left arrow', () => {
      expect(compareDates('2020-11-09T15:32:02.397Z', '2018-11-09T15:32:02.397Z')).to.eq('<=');
    });
    it('should show right arrow', () => {
      expect(compareDates('2019-11-09T15:32:02.397Z', '2019-11-10T15:32:02.397Z')).to.eq('=>');
    });
  });
})
