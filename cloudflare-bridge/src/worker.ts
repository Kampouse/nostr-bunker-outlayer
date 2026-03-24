/**
 * NEAR + Nostr Bunker - Cloudflare Worker Bridge
 * 
 * Security: Email only sent through authenticated sign_event with ["email", "..."] tag
 */

export interface Env {
  OUTLAYER_API_URL: string;
  PAYMENT_KEY: string;
  RATE_LIMIT_MAX: string;
  RATE_LIMIT_WINDOW: string;
  EMAIL_API_URL?: string;
}

const DEFAULT_EMAIL_API = 'https://api.outlayer.fastnear.com/call/zavodil.near/near-email';

interface NIP46Request {
  id: number | string;
  method: string;
  params: any[];
}

interface NIP46Response {
  id: number | string;
  result?: any;
  error?: string;
}

interface Metrics {
  requestsTotal: number;
  requestsSuccessful: number;
  requestsFailed: number;
  wsConnections: number;
  rateLimitedRequests: number;
  averageResponseTime: number;
  emailsSent: number;
}

const rateLimitStore = new Map<string, { count: number; resetTime: number }>();
const metrics: Metrics = {
  requestsTotal: 0,
  requestsSuccessful: 0,
  requestsFailed: 0,
  wsConnections: 0,
  rateLimitedRequests: 0,
  averageResponseTime: 0,
  emailsSent: 0,
};
const responseTimes: number[] = [];

function log(level: 'info' | 'warn' | 'error', message: string, data?: Record<string, any>) {
  console.log(JSON.stringify({ level, message, ...data, timestamp: new Date().toISOString() }));
}

function checkRateLimit(clientIp: string, max: number, windowSec: number): { allowed: boolean; retryAfter: number } {
  const now = Date.now();
  const entry = rateLimitStore.get(clientIp);
  if (!entry || now > entry.resetTime) {
    rateLimitStore.set(clientIp, { count: 1, resetTime: now + windowSec * 1000 });
    return { allowed: true, retryAfter: 0 };
  }
  if (entry.count >= max) return { allowed: false, retryAfter: Math.ceil((entry.resetTime - now) / 1000) };
  entry.count++;
  return { allowed: true, retryAfter: 0 };
}

async function sendEmail(env: Env, to: string, subject: string, body: string): Promise<{ success: boolean; error?: string }> {
  try {
    const res = await fetch(env.EMAIL_API_URL || DEFAULT_EMAIL_API, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Payment-Key': env.PAYMENT_KEY },
      body: JSON.stringify({ input: { action: 'send_email_plaintext', to, subject, body } }),
    });
    const data = await res.json() as any;
    return res.ok && data.status === 'completed' ? { success: true } : { success: false, error: data.output?.error || 'Unknown error' };
  } catch (e) {
    return { success: false, error: String(e) };
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const clientIp = request.headers.get('CF-Connecting-IP') || 'unknown';

    // WebSocket
    if (request.headers.get('Upgrade') === 'websocket') {
      const pair = new WebSocketPair();
      const [client, server] = Object.values(pair);
      server.accept();
      metrics.wsConnections++;
      log('info', 'WebSocket connected', { ip: clientIp });

      server.addEventListener('message', async (event: MessageEvent) => {
        const startTime = Date.now();
        try {
          const msg: NIP46Request = JSON.parse(event.data as string);
          if (!msg.method) throw new Error('Missing method');

          log('info', 'Request', { method: msg.method, ip: clientIp });

          // Forward to OutLayer
          const response = await fetch(env.OUTLAYER_API_URL, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-Payment-Key': env.PAYMENT_KEY },
            body: JSON.stringify({ input: msg }),
          });
          if (!response.ok) throw new Error('OutLayer error: ' + response.status);
          const result: NIP46Response = await response.json();

          const duration = Date.now() - startTime;
          responseTimes.push(duration);
          metrics.requestsTotal++;
          metrics.requestsSuccessful++;
          log('info', 'Success', { method: msg.method, duration, ip: clientIp });
          server.send(JSON.stringify(result));

          // If sign_event with email tag, send email
          if (msg.method === 'sign_event' && result.result && msg.params[0]?.tags) {
            const emailTag = msg.params[0].tags.find((t: string[]) => t[0] === 'email');
            if (emailTag) {
              const event = msg.params[0];
              const subject = event.tags.find((t: string[]) => t[0] === 'subject')?.[1] || 'Message from Nostr';
              const body = event.content + '\n\n---\nSigned by: ' + event.pubkey?.slice(0, 16) + '...';
              const emailResult = await sendEmail(env, emailTag[1], subject, body);
              if (emailResult.success) {
                metrics.emailsSent++;
                log('info', 'Email sent', { to: emailTag[1], ip: clientIp });
              } else {
                log('warn', 'Email failed', { error: emailResult.error, ip: clientIp });
              }
            }
          }
        } catch (e) {
          metrics.requestsTotal++;
          metrics.requestsFailed++;
          log('error', 'Error', { error: String(e), ip: clientIp });
          server.send(JSON.stringify({ id: 'error', error: String(e) }));
        }
      });

      server.addEventListener('close', () => {
        metrics.wsConnections--;
        log('info', 'WebSocket closed', { ip: clientIp });
      });

      return new Response(null, { status: 101, webSocket: client });
    }

    // Rate limiting for HTTP
    const rateLimit = checkRateLimit(clientIp, parseInt(env.RATE_LIMIT_MAX || '100'), parseInt(env.RATE_LIMIT_WINDOW || '60'));
    if (!rateLimit.allowed) {
      metrics.rateLimitedRequests++;
      return new Response(JSON.stringify({ error: 'Rate limit exceeded', retryAfter: rateLimit.retryAfter }), {
        status: 429,
        headers: { 'Content-Type': 'application/json', 'Retry-After': String(rateLimit.retryAfter) },
      });
    }

    // Health
    if (url.pathname === '/' || url.pathname === '/health') {
      return new Response(JSON.stringify({
        status: 'ok',
        version: '1.2.0',
        service: 'NEAR + Nostr Bunker',
        timestamp: new Date().toISOString(),
        endpoints: { websocket: 'wss://' + url.host, auth: 'https://' + url.host + '/auth/{account_id}' },
        note: 'Email only via authenticated sign_event with ["email", "..."] tag',
      }), { headers: { 'Content-Type': 'application/json' } });
    }

    // Metrics
    if (url.pathname === '/metrics') {
      const avgTime = responseTimes.length ? Math.round(responseTimes.reduce((a, b) => a + b, 0) / responseTimes.length) : 0;
      return new Response(JSON.stringify({ metrics: { ...metrics, averageResponseTime: avgTime } }), {
        headers: { 'Content-Type': 'application/json' },
      });
    }

    return new Response('Not Found', { status: 404 });
  },
};
