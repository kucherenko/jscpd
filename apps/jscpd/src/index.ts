import {
  getDefaultOptions,
  getModeHandler,
  IClone,
  IMapFrame,
  IOptions,
  IStatistic,
  IStore,
  Statistic,
} from "@jscpd/core";
import { grey, italic } from "colors/safe";
import {
  EntryWithContent,
  getFilesToDetect,
  InFilesDetector,
} from "@jscpd/finder";
import { initCli, initOptionsFromCli } from "./init";
import { printFiles, printOptions, printSupportedFormat } from "./print";
import { createHash } from "crypto";
import { getStore } from "./init/store";
import { getSupportedFormats, Tokenizer } from "@jscpd/tokenizer";
import { registerReporters } from "./init/reporters";
import { registerSubscribers } from "./init/subscribers";
import { registerHooks } from "./init/hooks";
import { readJSONSync } from "fs-extra";

const TIMER_LABEL = "time";
const DRY_REFACTORING_SKILL_INSTALL_COMMAND =
  "npx skills add kucherenko/jscpd --skill dry-refactoring";

export const detectClones = (
  opts: IOptions,
  store: IStore<IMapFrame> | undefined = undefined,
  statisticProvider: Statistic | undefined = undefined,
): Promise<IClone[]> => {
  const options: Partial<IOptions> = { ...getDefaultOptions(), ...opts };
  options.format = options.format || getSupportedFormats();
  options.mode = getModeHandler(options.mode);

  const files: EntryWithContent[] = getFilesToDetect(options);
  const hashFunction = (value: string): string => {
    return createHash("md5").update(value).digest("hex");
  };
  options.hashFunction = options.hashFunction || hashFunction;
  const currentStore: IStore<IMapFrame> = store || getStore(options.store, options.storePath);
  const statistic = statisticProvider || new Statistic();
  const tokenizer = new Tokenizer();
  const detector = new InFilesDetector(
    tokenizer,
    currentStore,
    statistic,
    options,
  );

  registerReporters(options, detector);
  registerSubscribers(options, detector);
  registerHooks(options, detector);

  if (!options.silent) {
    console.time(italic(grey(TIMER_LABEL)));
  }
  return detector.detect(files).then((clones: IClone[]) => {
    if (!options.silent) {
      console.timeEnd(italic(grey(TIMER_LABEL)));
      if (!options.noTips) {
        console.log("");
        console.log(
          grey(
            `💡 Auto-refactor with AI: ${DRY_REFACTORING_SKILL_INSTALL_COMMAND}`,
          ),
        );
        console.log(
          grey(
            "🎩 New: Gangsta Agents — discipline your AI coding → gangsta.page",
          ),
        );
        console.log(
          grey("💖 Support jscpd project → https://opencollective.com/jscpd"),
        );
      }
    }
    return clones;
  });
};

export const detectClonesAndStatistic = (
  opts: IOptions,
  store: IStore<IMapFrame> | undefined = undefined,
): Promise<{ clones: IClone[]; statistic: IStatistic }> => {
  const statistic = new Statistic();
  return detectClones(opts, store, statistic).then((clones: IClone[]) => ({
    clones,
    statistic: statistic.getStatistic(),
  }));
};

export async function jscpd(
  argv: string[],
  exitCallback?: (code: number) => {},
) {
  const packageJson = readJSONSync(__dirname + "/../package.json");

  const cli = initCli(packageJson, argv);

  const options: IOptions = initOptionsFromCli(cli);

  if (options.list) {
    printSupportedFormat();
  }

  if (options.debug) {
    printOptions(options);
  }

  if (!options.path || options.path.length === 0) {
    options.path = [process.cwd()];
  }

  if (options.debug) {
    const files: EntryWithContent[] = getFilesToDetect(options);
    printFiles(files);
    return Promise.resolve([]);
  } else {
    const store = getStore(options.store, options.storePath);
    return detectClones(options, store)
      .then((clones) => {
        if (clones.length > 0) {
          exitCallback?.(options.exitCode || 0);
        }
        return clones;
      })
      .finally(() => {
        store.close();
      });
  }
}
