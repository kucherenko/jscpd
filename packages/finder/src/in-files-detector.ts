import {
	Detector,
	DetectorEvents,
	IClone,
	ICloneValidator,
	IHandler,
	IOptions,
	IStore,
	ISubscriber,
	MemoryStore,
	Statistic,
} from '@jscpd/core';
import {getFormatByFile} from '@jscpd/tokenizer';
import {EntryWithContent, IHook, IReporter} from './interfaces';
import {BlamerHook, FragmentsHook} from './hooks';
import {VerboseSubscriber} from './subscribers';
import {SkipLocalValidator} from './validators';

export class InFilesDetector {

	private readonly store: IStore<any>;
	private readonly statistic: Statistic;

	private readonly reporters: IReporter[] = [];
	private readonly subscribes: ISubscriber[] = [];
	private readonly postHooks: IHook[] = [];

	constructor(
		private options: IOptions,
		statistic: Statistic,
		store: IStore<any> | undefined = undefined,
	) {
		this.store = store || new MemoryStore();
		this.statistic = statistic || new Statistic(options);
		this.registerSubscriber(this.statistic);

		this.registerHook(new FragmentsHook());
		if (this.options.blame) {
			this.registerHook(new BlamerHook());
		}
		if (this.options.verbose) {
			this.registerSubscriber(new VerboseSubscriber(this.options));
		}
	}

	registerReporter(reporter: IReporter) {
		this.reporters.push(reporter);
	}

	registerSubscriber(subscriber: ISubscriber) {
		this.subscribes.push(subscriber);
	}

	registerHook(hook: IHook) {
		this.postHooks.push(hook);
	}

	detect(files: EntryWithContent[]): Promise<IClone[]> {
		const options = this.options;
		const hooks = [...this.postHooks];
		const store = this.store;
		const validators: ICloneValidator[] = [];

		if (options.skipLocal) {
			validators.push(new SkipLocalValidator());
		}

		const detector = new Detector(options, store, validators);

		this.subscribes.forEach((listener: ISubscriber) => {
			Object
				.entries(listener.subscribe())
				.map(([event, handler]: [DetectorEvents, IHandler]) => detector.on(event, handler));
		});

		const detect = (entry: EntryWithContent, clones: IClone[] = []) => {
			const {path, content} = entry;
			const format: string = getFormatByFile(path, options.formatsExts) as string;
			return detector
				.detect(path, content, format)
				.then((clns: IClone[]) => {
					clones.push(...clns);
					const file = files.pop();
					if (file) {
						return detect(file, clones);
					}
					return clones;
				});
		};

		const processHooks = (hook: IHook, detectedClones: IClone[]): Promise<IClone[]> => {
			return hook
				.process(detectedClones)
				.then((clones: IClone[]) => {
					const nextHook: IHook = hooks.pop();
					if (nextHook) {
						return processHooks(nextHook, clones);
					}
					return clones;
				});
		}

		return detect(files.pop())
			.then((clones: IClone[]) => {
				const hook = hooks.pop();
				if (hook) {
					return processHooks(hook, clones)
				}
				return clones;
			})
			.then((clones: IClone[]) => {
				const statistic = this.statistic.getStatistic();
				this.reporters.forEach((reporter: IReporter) => {
					reporter.report(clones, statistic);
				});
				return clones;
			});
	}
}
