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

    await server.getService().initialize(server['options'].jscpdOptions);

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

  describe('Snippet Isolation and Memory Management', () => {
    it('should isolate snippet tokens between requests', async () => {
      // First request with unique snippet
      const snippet1 = `
function uniqueFunction1_test() {
  const x1 = 1;
  const y1 = 2;
  const z1 = 3;
  const a1 = 4;
  const b1 = 5;
  const c1 = 6;
  return x1 + y1 + z1;
}
      `.trim();

      const response1 = await request
        .post('/api/check')
        .send({ code: snippet1, format: 'javascript' });

      expect(response1.status).toBe(200);

      // Second request with the same snippet should not see snippet1's tokens
      // (it should only detect duplications against the project, not previous snippets)
      const snippet2 = snippet1;
      const response2 = await request
        .post('/api/check')
        .send({ code: snippet2, format: 'javascript' });

      expect(response2.status).toBe(200);

      // Both responses should be identical since snippets are the same
      // and should not detect each other as duplications
      expect(response2.body.duplications).toEqual(response1.body.duplications);
    });

    it('should not contaminate project store with snippet tokens', async () => {
      // Check a snippet
      const testSnippet = `
function testContamination() {
  const contamination1 = 1;
  const contamination2 = 2;
  const contamination3 = 3;
  const contamination4 = 4;
  const contamination5 = 5;
  const contamination6 = 6;
  return contamination1 + contamination2;
}
      `.trim();

      const response1 = await request
        .post('/api/check')
        .send({ code: testSnippet, format: 'javascript' });

      expect(response1.status).toBe(200);
      const initialDuplications = response1.body.duplications.length;

      // Check the same snippet again - should produce identical results
      // (snippet tokens from first request should not contaminate the store)
      const response2 = await request
        .post('/api/check')
        .send({ code: testSnippet, format: 'javascript' });

      expect(response2.status).toBe(200);
      expect(response2.body.duplications.length).toBe(initialDuplications);

      // The duplications should be against project files, not the previous snippet
      if (response2.body.duplications.length > 0) {
        const hasSnippetPath = response2.body.duplications.some(
          (dup: any) => dup.codebaseLocation.file.includes('<snippet>')
        );
        expect(hasSnippetPath).toBe(false);
      }
    });

    it('should handle concurrent snippet checks without cross-contamination', async () => {
      const snippet1 = `
function concurrent1() {
  const concurrent_var_1 = 1;
  const concurrent_var_2 = 2;
  const concurrent_var_3 = 3;
  const concurrent_var_4 = 4;
  const concurrent_var_5 = 5;
  const concurrent_var_6 = 6;
  return concurrent_var_1 + concurrent_var_2;
}
      `.trim();

      const snippet2 = `
function concurrent2() {
  const different_var_1 = "a";
  const different_var_2 = "b";
  const different_var_3 = "c";
  const different_var_4 = "d";
  const different_var_5 = "e";
  const different_var_6 = "f";
  return different_var_1 + different_var_2;
}
      `.trim();

      // Send multiple concurrent requests
      const [response1, response2, response3] = await Promise.all([
        request.post('/api/check').send({ code: snippet1, format: 'javascript' }),
        request.post('/api/check').send({ code: snippet2, format: 'javascript' }),
        request.post('/api/check').send({ code: snippet1, format: 'javascript' }),
      ]);

      expect(response1.status).toBe(200);
      expect(response2.status).toBe(200);
      expect(response3.status).toBe(200);

      // First and third requests (same snippet) should have identical results
      expect(response1.body.duplications).toEqual(response3.body.duplications);

      // None should detect duplications against snippet paths
      [response1, response2, response3].forEach((response) => {
        if (response.body.duplications.length > 0) {
          const hasSnippetPath = response.body.duplications.some(
            (dup: any) => dup.codebaseLocation.file.includes('<snippet>')
          );
          expect(hasSnippetPath).toBe(false);
        }
      });
    });

    it('should properly clean up ephemeral store after request', async () => {
      const memoryBefore = process.memoryUsage().heapUsed;

      // Perform many snippet checks
      const promises: Promise<any>[] = [];
      for (let i = 0; i < 10; i++) {
        const snippet = `
function memoryTest${i}() {
  const var1_${i} = ${i};
  const var2_${i} = ${i * 2};
  const var3_${i} = ${i * 3};
  const var4_${i} = ${i * 4};
  const var5_${i} = ${i * 5};
  const var6_${i} = ${i * 6};
  return var1_${i} + var2_${i};
}
        `.trim();

        promises.push(
          request.post('/api/check').send({ code: snippet, format: 'javascript' })
        );
      }

      const responses = await Promise.all(promises);
      responses.forEach((response) => {
        expect(response.status).toBe(200);
      });

      // Force garbage collection if available
      if (global.gc) {
        global.gc();
      }

      const memoryAfter = process.memoryUsage().heapUsed;
      const memoryGrowth = memoryAfter - memoryBefore;

      // Memory growth should be reasonable (not unbounded)
      // Note: This is a heuristic test and might need adjustment
      const reasonableGrowthLimit = 50 * 1024 * 1024; // 50MB
      expect(memoryGrowth).toBeLessThan(reasonableGrowthLimit);
    });
  });
});
