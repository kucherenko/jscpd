export class Emitter {
  constructor() {
    this.listeners = new Map();
  }

  on(event, handler) {
    const handlers = this.listeners.get(event) || [];
    handlers.push(handler);
    this.listeners.set(event, handlers);
    return () => this.off(event, handler);
  }

  off(event, handler) {
    const handlers = this.listeners.get(event) || [];
    this.listeners.set(event, handlers.filter((h) => h !== handler));
  }

  emit(event, payload) {
    for (const handler of this.listeners.get(event) || []) {
      handler(payload);
    }
  }
}
