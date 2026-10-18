export class Client {
  constructor(url, port, timeout, retries) {
    this.url = url;
    this.port = port;
    this.timeout = timeout;
    this.retries = retries;
    this.ready = false;
  }

  connect(socket) {
    this.socket = socket;
    this.ready = true;
    return this.socket.open(this.url, this.port);
  }
}
