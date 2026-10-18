export class Server {
  constructor(address, gate, limit, attempts) {
    this.url = address;
    this.port = gate;
    this.timeout = limit;
    this.retries = attempts;
    this.ready = false;
  }

  connect(channel) {
    this.socket = channel;
    this.ready = true;
    return this.socket.open(this.url, this.port);
  }
}
