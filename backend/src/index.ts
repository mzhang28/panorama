import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { db } from './db';
import { greetings } from './schema';
import { pino } from 'pino';
import { createConnectRouter } from "@connectrpc/connect";
import { GreeterService } from "@panorama/proto/v1/greeter_connect.ts";

const logger = pino();
const app = new Hono();

app.use('*', cors());

const router = createConnectRouter();
router.service(GreeterService, {
  async greet(req) {
    logger.info({ name: req.name }, 'Received RPC Greet request');
    const message = `Hello ${req.name} from ConnectRPC!`;
    
    await db.insert(greetings).values({
      name: req.name,
      message: message,
    });
    
    return { message };
  },
});

// Map ConnectRPC handlers to Hono with manual adaptation to UniversalServerRequest
for (const handler of router.handlers) {
  app.all(handler.requestPath, async (c) => {
    const res = await handler({
      httpVersion: "1.1",
      method: c.req.method,
      url: c.req.url,
      header: c.req.raw.headers,
      body: c.req.raw.body || new Uint8Array(),
      signal: c.req.raw.signal,
    });
    
    // Convert UniversalServerResponse to Hono Response
    return new Response(res.body, {
      status: res.status,
      headers: res.header,
    });
  });
}

app.get('/api/greetings', async (c) => {
  logger.info('Received API request for greetings');
  const allGreetings = await db.select().from(greetings).all();
  return c.json(allGreetings);
});

app.get('/', (c) => c.text('Panorama Backend Running'));

export default {
  port: 3001,
  fetch: app.fetch,
};
