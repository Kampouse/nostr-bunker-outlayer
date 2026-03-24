/**
 * NEAR + Nostr Bunker - Cloudflare Worker Bridge
 * With Rate Limiting and Improved Error Handling
 */

export interface Env {
  OUTLAYER_API_URL: string;
  PAYMENT_KEY: string;
  RATE_LIMIT_MAX: string; // Max requests per minute
  RATE_LIMIT_WINDOW: string; // Window in seconds
}

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

interface RateLimitEntry {
  count: number;
  resetTime: number;
}

// Simple in-memory rate limiting (resets on worker restart)
const rateLimitStore = new Map<string, RateLimitEntry>();

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // Rate limiting
    if (request.headers.get('Upgrade') !== 'websocket') {
      const clientIp = request.headers.get('CF-Connecting-IP') || 'unknown';
      const rateLimitResult = checkRateLimit(
        clientIp,
        parseInt(env.RATE_LIMIT_MAX || '100'),
        parseInt(env.RATE_LIMIT_WINDOW || '60')
      );

      if (!rateLimitResult.allowed) {
        return new Response(JSON.stringify({
          error: 'Rate limit exceeded',
          retryAfter: rateLimitResult.retryAfter,
        }), {
          status: 429,
          headers: {
            'Content-Type': 'application/json',
            'Retry-After': String(rateLimitResult.retryAfter),
          },
        });
      }
    }

    // WebSocket upgrade for NIP-46
    if (request.headers.get('Upgrade') === 'websocket') {
      return this.handleWebSocket(request, env);
    }

    // Health check endpoint
    if (url.pathname === '/' || url.pathname === '/health') {
      return new Response(JSON.stringify({
        status: 'ok',
        version: '1.1.0',
        service: 'NEAR + Nostr Bunker Bridge',
        rateLimit: {
          max: env.RATE_LIMIT_MAX || 100,
          window: env.RATE_LIMIT_WINDOW || 60,
        },
        endpoints: {
          websocket: `wss://${url.host}`,
          health: `https://${url.host}/health`,
        },
      }), {
        headers: { 'Content-Type': 'application/json' },
      });
    }

    // Metrics endpoint (basic)
    if (url.pathname === '/metrics') {
      return new Response(JSON.stringify({
        rateLimit: {
          activeClients: rateLimitStore.size,
          totalRequests: Array.from(rateLimitStore.values())
            .reduce((sum, entry) => sum + entry.count, 0),
        },
      }), {
        headers: { 'Content-Type': 'application/json' },
      });
    }

    // Auth page
    if (url.pathname.startsWith('/auth/')) {
      return this.handleAuth(url);
    }

    return new Response('Not Found', { status: 404 });
  },

  async handleWebSocket(request: Request, env: Env): Promise<Response> {
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);

    server.accept();

    server.addEventListener('message', async (event: MessageEvent) => {
      try {
        const msg: NIP46Request = JSON.parse(event.data as string);

        // Validate request
        if (!msg.method) {
          throw new Error('Missing method field');
        }

        if (!Array.isArray(msg.params)) {
          throw new Error('Params must be an array');
        }

        // Log for monitoring
        console.log(JSON.stringify({
          timestamp: new Date().toISOString(),
          method: msg.method,
          id: msg.id,
        }));

        // Forward to OutLayer
        const response = await fetch(env.OUTLAYER_API_URL, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-Payment-Key': env.PAYMENT_KEY,
          },
          body: JSON.stringify({
            input: msg,
          }),
        });

        if (!response.ok) {
          throw new Error(`OutLayer error: ${response.status} ${response.statusText}`);
        }

        const result: NIP46Response = await response.json();

        // Send back to client
        server.send(JSON.stringify(result));

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';

        console.error('WebSocket error:', errorMessage);

        server.send(JSON.stringify({
          id: 'error',
          error: errorMessage,
        }));
      }
    });

    server.addEventListener('close', (event: CloseEvent) => {
      console.log('WebSocket closed:', {
        code: event.code,
        reason: event.reason,
      });
    });

    server.addEventListener('error', (error: Event) => {
      console.error('WebSocket error:', error);
    });

    return new Response(null, {
      status: 101,
      webSocket: client,
    });
  },

  async handleAuth(url: URL): Promise<Response> {
    const accountId = url.pathname.split('/')[2] || 'unknown';

    const html = `
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Authorize Nostr - ${accountId}</title>
  <script src="https://cdn.jsdelivr.net/npm/near-api-js@3.0.0/dist/near-api-js.min.js"></script>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    }
    .container {
      background: white;
      border-radius: 20px;
      padding: 40px;
      max-width: 400px;
      width: 90%;
      text-align: center;
      box-shadow: 0 20px 60px rgba(0,0,0,0.3);
    }
    h1 { margin-bottom: 10px; font-size: 28px; }
    p { color: #666; margin-bottom: 20px; }
    button {
      width: 100%;
      padding: 16px;
      background: #007AFF;
      color: white;
      border: none;
      border-radius: 12px;
      font-size: 18px;
      font-weight: 600;
      cursor: pointer;
      transition: transform 0.2s;
      margin-bottom: 10px;
    }
    button:hover { transform: scale(1.05); }
    button:active { transform: scale(0.95); }
    button:disabled {
      background: #ccc;
      cursor: not-allowed;
      transform: none;
    }
    .account { font-weight: bold; color: #007AFF; }
    .status { margin-top: 20px; padding: 16px; border-radius: 8px; }
    .success { background: #d4edda; color: #155724; }
    .error { background: #f8d7da; color: #721c24; }
    .info { background: #d1ecf1; color: #0c5460; font-size: 14px; }
    .loading { opacity: 0.6; }
  </style>
</head>
<body>
  <div class="container">
    <h1>🔐 Authorize Nostr</h1>
    <p>Login with NEAR to enable Nostr signing</p>
    <p>Account: <span class="account">${accountId}</span></p>

    <button id="loginBtn" onclick="login()">Login with NEAR</button>

    <div class="status info">
      This creates a 30-day session
    </div>

    <div id="status"></div>
  </div>

  <script>
    async function login() {
      const btn = document.getElementById('loginBtn');
      btn.disabled = true;
      btn.textContent = 'Connecting...';

      try {
        const near = await nearApi.connect({
          networkId: 'mainnet',
          nodeUrl: 'https://rpc.mainnet.near.org',
          walletUrl: 'https://wallet.mainnet.near.org',
        });

        const wallet = new nearApi.WalletConnection(near, 'nostr-bunker');

        if (!wallet.isSignedIn()) {
          await wallet.requestSignIn({
            contractId: 'v1.signer',
            methodNames: ['sign', 'derived_public_key'],
          });
        }

        const loggedInAccount = wallet.getAccountId();

        if (loggedInAccount !== '${accountId}') {
          showError('Wrong account! Please login as ${accountId}');
          btn.disabled = false;
          btn.textContent = 'Login with NEAR';
          return;
        }

        showSuccess('✓ Authorized! You can now close this page.');

        // Auto-close after 2 seconds
        setTimeout(() => window.close(), 2000);

      } catch (error) {
        showError('Error: ' + error.message);
        btn.disabled = false;
        btn.textContent = 'Login with NEAR';
      }
    }

    function showSuccess(message) {
      document.getElementById('status').innerHTML =
        '<div class="status success">' + message + '</div>';
    }

    function showError(message) {
      document.getElementById('status').innerHTML =
        '<div class="status error">' + message + '</div>';
    }
  </script>
</body>
</html>
    `;

    return new Response(html, {
      headers: { 'Content-Type': 'text/html' },
    });
  },
};

// Rate limiting implementation
function checkRateLimit(
  clientIp: string,
  maxRequests: number,
  windowSeconds: number
): { allowed: boolean; retryAfter: number } {
  const now = Date.now();
  const windowMs = windowSeconds * 1000;

  const entry = rateLimitStore.get(clientIp);

  if (!entry || now > entry.resetTime) {
    // New window
    rateLimitStore.set(clientIp, {
      count: 1,
      resetTime: now + windowMs,
    });
    return { allowed: true, retryAfter: 0 };
  }

  if (entry.count >= maxRequests) {
    // Rate limited
    const retryAfter = Math.ceil((entry.resetTime - now) / 1000);
    return { allowed: false, retryAfter };
  }

  // Increment count
  entry.count++;
  return { allowed: true, retryAfter: 0 };
}
