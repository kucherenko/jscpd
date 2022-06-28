import {join} from 'path';
import {IClone, IOptions, IStatistic} from '@jscpd/core';
import {IReporter, JsonReporter} from "@jscpd/finder";
import {copySync, writeFileSync, readFileSync} from "fs-extra";
import {green, red} from "colors/safe";

function escapeHtml(value) {
  if (typeof value !== 'string') {
    return value;
  }
  return value.replace(/[&<>`"'/]/g, function(result) {
    return {
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '`': '&#x60;',
      '"': '&quot;',
      "'": '&#x27;',
      '/': '&#x2f;',
    }[result];
  })
}

export default class HtmlReporter implements IReporter {
  constructor(private options: IOptions) {
  }

  public report(clones: IClone[], statistic: IStatistic): void {
    const jsonReporter = new JsonReporter(this.options);
    const json = jsonReporter.generateJson(clones, statistic);
    json.duplicates.forEach(item => {
      item.fragment = escapeHtml(item.fragment);
    });
    if (this.options.output) {
      const src = join(__dirname, '../html/');
      const destination = join(this.options.output, 'html/');
      try {
        copySync(src, destination, {overwrite: true});
        const index = join(destination, 'index.html');
        const html = readFileSync(index).toString();
        writeFileSync(index, html.replace(
          '<body>',
          `<body><script>
                       // <!--
                       window.initialData = ${JSON.stringify(json, null, '  ')};
                       // -->
                       </script>`
        ))
        writeFileSync(join(destination, 'jscpd-report.json'), JSON.stringify(json, null, '  '));
        console.log(green(`HTML report saved to ${join(this.options.output, 'html/')}`));
      } catch (e) {
        console.log(red(e))
      }
    }
  }
}
