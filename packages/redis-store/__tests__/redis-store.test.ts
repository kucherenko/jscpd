import {expect} from 'chai';
import sinon = require('sinon');

const proxyquire = require('proxyquire');

const ioredis = sinon.stub();

ioredis.prototype.set = sinon.spy();
ioredis.prototype.disconnect = sinon.spy();

ioredis.prototype.get = sinon.stub()
  .returns(Promise.resolve('"test"'));


const redisStore = proxyquire('..', {
  ioredis,
})

describe('Redis Store', () => {

  it('should set data to store', () => {
    const store = new redisStore.default();
    store.namespace('test')
    store.set('key', 'data');
    expect(ioredis.prototype.set.calledWith('test:key', JSON.stringify('data'))).to.be.ok;
  });

  it('should get data from redis', async () => {
    const store = new redisStore.default();
    const data = await store.get('key');
    expect(data).to.equal('test');
  });

  it('should disconnect on clone', () => {
    const store = new redisStore.default();
    store.close();
    expect(ioredis.prototype.disconnect.called).to.be.ok;
  });
});
