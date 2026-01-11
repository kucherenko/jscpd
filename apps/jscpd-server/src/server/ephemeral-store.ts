import { IMapFrame, IStore } from '@jscpd/core';

/**
 * A hybrid store that delegates reads to a source store (for project data)
 * and writes to an ephemeral store (for snippet data), ensuring snippet
 * tokens don't contaminate the shared project store and are automatically
 * discarded after detection.
 */
export class EphemeralHybridStore implements IStore<IMapFrame> {
  constructor(
    private readonly sourceStore: IStore<IMapFrame>,
    private readonly ephemeralStore: IStore<IMapFrame>
  ) {}

  namespace(name: string): void {
    this.sourceStore.namespace(name);
    this.ephemeralStore.namespace(name);
  }

  async get(key: string): Promise<IMapFrame> {
    try {
      return await this.ephemeralStore.get(key);
    } catch {
      return this.sourceStore.get(key);
    }
  }

  async set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return this.ephemeralStore.set(key, value);
  }

  close(): void {
    this.ephemeralStore.close();
  }
}

