import path from 'node:path';
import { fileURLToPath } from 'node:url';
import Fastify from 'fastify';
import autoload from '@fastify/autoload';

const here = path.dirname(fileURLToPath(import.meta.url));
const app = Fastify({ logger: true });

// Neither directory is imported: the loader lists them at startup.
app.register(autoload, { dir: path.join(here, 'plugins') });
app.register(autoload, { dir: path.join(here, 'routes'), options: { prefix: '/v1' } });

app.listen({ port: Number(process.env.PORT ?? 4100) });
