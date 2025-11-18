import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import supertest from 'supertest';
import { JscpdServer } from '../src/server';
import { join } from 'path';

describe('JSCPD Server', () => {
  let server: JscpdServer;
  let request: supertest.SuperTest<supertest.Test>;

  beforeAll(async () => {
    const testDirectory = join(__dirname, '../../..', 'fixtures', 'javascript');
    server = new JscpdServer(testDirectory, {
      port: 0,
      jscpdOptions: {
        minLines: 5,
        minTokens: 50,
      },
    });

    await server.getService().initialize({
      minLines: 5,
      minTokens: 50,
    });

    request = supertest(server.getApp());
  });

  afterAll(async () => {
    await server.stop();
  });

  describe('GET /', () => {
    it('should return API information', async () => {
      const response = await request.get('/');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('name', 'jscpd-server');
      expect(response.body).toHaveProperty('endpoints');
    });
  });

  describe('GET /api/health', () => {
    it('should return server health status', async () => {
      const response = await request.get('/api/health');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('status');
      expect(response.body).toHaveProperty('workingDirectory');
    });
  });

  describe('GET /api/stats', () => {
    it('should return project statistics', async () => {
      const response = await request.get('/api/stats');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('statistics');
      expect(response.body).toHaveProperty('timestamp');
      expect(response.body.statistics).toHaveProperty('total');
      expect(response.body.statistics.total).toHaveProperty('lines');
      expect(response.body.statistics.total).toHaveProperty('sources');
    });
  });

  describe('POST /api/check', () => {
    it('should check code snippet for duplications', async () => {
      const codeSnippet = `
function test() {
  const a = 1;
  const b = 2;
  const c = 3;
  const d = 4;
  const e = 5;
  const f = 6;
  return a + b + c + d + e + f;
}
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: codeSnippet,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
      expect(response.body).toHaveProperty('statistics');
      expect(response.body.statistics).toHaveProperty('totalDuplications');
      expect(response.body.statistics).toHaveProperty('duplicatedLines');
      expect(response.body.statistics).toHaveProperty('totalLines');
      expect(response.body.statistics).toHaveProperty('percentageDuplicated');
      expect(Array.isArray(response.body.duplications)).toBe(true);
    });

    it('should return validation error for missing code', async () => {
      const response = await request
        .post('/api/check')
        .send({});

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body).toHaveProperty('message');
      expect(response.body.message).toContain('code');
    });

    it('should return validation error for empty code', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: '   ',
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body.message).toContain('empty');
    });

    it('should return validation error for non-string code', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: 123,
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body.message).toContain('string');
    });

    it('should handle code with no duplications', async () => {
      const uniqueCode = `
function veryUniqueFunction_${Date.now()}() {
  const uniqueVar1 = "unique_${Date.now()}_1";
  const uniqueVar2 = "unique_${Date.now()}_2";
  const uniqueVar3 = "unique_${Date.now()}_3";
  const uniqueVar4 = "unique_${Date.now()}_4";
  const uniqueVar5 = "unique_${Date.now()}_5";
  const uniqueVar6 = "unique_${Date.now()}_6";
  return uniqueVar1 + uniqueVar2 + uniqueVar3;
}
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: uniqueCode,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body.duplications).toHaveLength(0);
      expect(response.body.statistics.totalDuplications).toBe(0);
      expect(response.body.statistics.percentageDuplicated).toBe(0);
    });

    it('should accept filename parameter', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: 'console.log("test");',
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
    });

    it('should handle different languages', async () => {
      const pythonCode = `
def hello():
    x = 1
    y = 2
    z = 3
    a = 4
    b = 5
    c = 6
    return x + y + z
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: pythonCode,
          format: 'python',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
    });
  });

  describe('404 Not Found', () => {
    it('should return 404 for unknown routes', async () => {
      const response = await request.get('/api/unknown');

      expect(response.status).toBe(404);
      expect(response.body).toHaveProperty('error', 'NotFound');
    });
  });
});
