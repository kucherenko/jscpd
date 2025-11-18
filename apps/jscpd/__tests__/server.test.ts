import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import supertest from 'supertest';
import { JscpdServer } from '../src/server';
import { join } from 'path';

describe('JSCPD Server', () => {
  let server: JscpdServer;
  let request: ReturnType<typeof supertest>;

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
      expect(response.body).toHaveProperty('version');
      expect(response.body).toHaveProperty('endpoints');
      expect(response.body).toHaveProperty('documentation');
      expect(response.body.endpoints).toHaveProperty('POST /api/check');
      expect(response.body.endpoints).toHaveProperty('GET /api/stats');
      expect(response.body.endpoints).toHaveProperty('GET /api/health');
    });
  });

  describe('GET /api/health', () => {
    it('should return server health status', async () => {
      const response = await request.get('/api/health');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('status');
      expect(response.body).toHaveProperty('workingDirectory');
      expect(response.body).toHaveProperty('lastScanTime');
      expect(['ready', 'initializing']).toContain(response.body.status);
      expect(typeof response.body.workingDirectory).toBe('string');
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
      expect(typeof response.body.timestamp).toBe('string');
    });

    it('should return valid statistics structure', async () => {
      const response = await request.get('/api/stats');

      expect(response.status).toBe(200);
      const { statistics } = response.body;
      expect(statistics).toBeDefined();
      expect(typeof statistics.total.lines).toBe('number');
      expect(typeof statistics.total.sources).toBe('number');
      expect(statistics.total.lines).toBeGreaterThanOrEqual(0);
      expect(statistics.total.sources).toBeGreaterThanOrEqual(0);
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
          format: 'javascript',
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
          format: 'javascript',
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body.message).toContain('string');
    });

    it('should return validation error for missing format', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: 'console.log("test");',
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body).toHaveProperty('message');
      expect(response.body.message).toContain('format');
    });

    it('should return validation error for non-string format', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: 'console.log("test");',
          format: 123,
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error', 'ValidationError');
      expect(response.body.message).toContain('format');
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

    it('should detect duplications against codebase', async () => {
      const codeSnippet = `
const a = 1;
const b = 2;
const c = 3;
const d = 4;
const e = 5;
const f = 6;
const g = 7;
const h = 8;
const i = 9;
const j = 10;
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: codeSnippet,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
      expect(Array.isArray(response.body.duplications)).toBe(true);

      if (response.body.duplications.length > 0) {
        const firstDup = response.body.duplications[0];
        expect(firstDup).toHaveProperty('snippetLocation');
        expect(firstDup).toHaveProperty('codebaseLocation');
        expect(firstDup).toHaveProperty('linesCount');
        expect(firstDup.snippetLocation).toHaveProperty('startLine');
        expect(firstDup.snippetLocation).toHaveProperty('endLine');
        expect(firstDup.codebaseLocation).toHaveProperty('file');
        expect(firstDup.codebaseLocation).toHaveProperty('startLine');
        expect(firstDup.codebaseLocation).toHaveProperty('endLine');
      }
    });

    it('should handle large code snippets', async () => {
      const largeCode = Array(100)
        .fill(null)
        .map((_, i) => `const variable${i} = ${i};`)
        .join('\n');

      const response = await request
        .post('/api/check')
        .send({
          code: largeCode,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
      expect(response.body.statistics.totalLines).toBe(100);
    });

    it('should calculate duplication percentage correctly', async () => {
      const codeSnippet = `
function test() {
  const a = 1;
  const b = 2;
  const c = 3;
  const d = 4;
  const e = 5;
  return a + b + c + d + e;
}
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: codeSnippet,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body.statistics).toHaveProperty('percentageDuplicated');
      expect(typeof response.body.statistics.percentageDuplicated).toBe('number');
      expect(response.body.statistics.percentageDuplicated).toBeGreaterThanOrEqual(0);
      expect(response.body.statistics.percentageDuplicated).toBeLessThanOrEqual(100);
    });
  });

  describe('404 Not Found', () => {
    it('should return 404 for unknown routes', async () => {
      const response = await request.get('/api/unknown');

      expect(response.status).toBe(404);
      expect(response.body).toHaveProperty('error', 'NotFound');
      expect(response.body.message).toContain('GET /api/unknown');
    });
  });

  describe('Server Lifecycle', () => {
    it('should start and stop server properly', async () => {
      const testServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
        jscpdOptions: {
          minLines: 5,
          minTokens: 50,
        },
      });

      await testServer.getService().initialize({
        minLines: 5,
        minTokens: 50,
      });

      await testServer.start();

      const testRequest = supertest(testServer.getApp());
      const response = await testRequest.get('/api/health');
      expect(response.status).toBe(200);
      expect(response.body.status).toBe('ready');

      await testServer.stop();
    });

    it('should handle multiple stop calls gracefully', async () => {
      const testServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
      });

      await testServer.stop();
      await testServer.stop();
    });
  });

  describe('Server Not Initialized', () => {
    it('should return error when checking snippet before initialization', async () => {
      const uninitializedServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
      });

      const uninitializedRequest = supertest(uninitializedServer.getApp());
      const response = await uninitializedRequest
        .post('/api/check')
        .send({
          code: 'console.log("test");',
          format: 'javascript',
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error');
      expect(response.body.message).toContain('not initialized');

      await uninitializedServer.stop();
    });

    it('should return 503 when stats requested before initialization', async () => {
      const uninitializedServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
      });

      const uninitializedRequest = supertest(uninitializedServer.getApp());
      const response = await uninitializedRequest.get('/api/stats');

      expect(response.status).toBe(503);
      expect(response.body).toHaveProperty('error', 'NotReady');
      expect(response.body.message).toContain('initializing');

      await uninitializedServer.stop();
    });

    it('should show initializing status during scan', async () => {
      const scanningServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
      });

      const scanningRequest = supertest(scanningServer.getApp());
      const response = await scanningRequest.get('/api/health');

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('status');
      expect(['ready', 'initializing']).toContain(response.body.status);

      await scanningServer.stop();
    });
  });

  describe('Content Type Headers', () => {
    it('should return JSON content type', async () => {
      const response = await request.get('/');
      expect(response.headers['content-type']).toMatch(/application\/json/);
    });

    it('should return JSON content type for API endpoints', async () => {
      const response = await request.get('/api/health');
      expect(response.headers['content-type']).toMatch(/application\/json/);
    });

    it('should return JSON content type for errors', async () => {
      const response = await request.get('/api/unknown');
      expect(response.headers['content-type']).toMatch(/application\/json/);
    });
  });

  describe('Error Response Structure', () => {
    it('should return consistent error structure for validation errors', async () => {
      const response = await request
        .post('/api/check')
        .send({
          code: 123,
          format: 'javascript',
        });

      expect(response.status).toBe(400);
      expect(response.body).toHaveProperty('error');
      expect(response.body).toHaveProperty('message');
      expect(response.body).toHaveProperty('statusCode', 400);
      expect(typeof response.body.error).toBe('string');
      expect(typeof response.body.message).toBe('string');
    });

    it('should return consistent error structure for not found', async () => {
      const response = await request.get('/api/nonexistent');

      expect(response.status).toBe(404);
      expect(response.body).toHaveProperty('error', 'NotFound');
      expect(response.body).toHaveProperty('message');
      expect(response.body).toHaveProperty('statusCode', 404);
    });

    it('should return consistent error structure for not ready state', async () => {
      const uninitializedServer = new JscpdServer(join(__dirname, '../../..', 'fixtures', 'javascript'), {
        port: 0,
      });

      const uninitializedRequest = supertest(uninitializedServer.getApp());
      const response = await uninitializedRequest.get('/api/stats');

      expect(response.status).toBe(503);
      expect(response.body).toHaveProperty('error', 'NotReady');
      expect(response.body).toHaveProperty('message');
      expect(response.body).toHaveProperty('statusCode', 503);

      await uninitializedServer.stop();
    });
  });

  describe('Request Body Handling', () => {
    it('should accept URL encoded bodies', async () => {
      const response = await request
        .post('/api/check')
        .type('form')
        .send({
          code: 'console.log("test");',
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
    });

    it('should handle special characters in code', async () => {
      const codeWithSpecialChars = `
const str = "Hello, 世界! 🌍";
const regex = /[a-z]+/gi;
const template = \`\${str}\`;
      `.trim();

      const response = await request
        .post('/api/check')
        .send({
          code: codeWithSpecialChars,
          format: 'javascript',
        });

      expect(response.status).toBe(200);
      expect(response.body).toHaveProperty('duplications');
    });
  });
});
