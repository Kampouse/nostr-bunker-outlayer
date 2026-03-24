/**
 * NEAR + Nostr Bunker - Cloudflare Worker Bridge
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

interface OutlayerResponse {
  call_id: string;
  status: string;
  output?: {
    id: number;
    result?: any;
    error?: string;
  };
  error?: string;
}

function log(level: string, message: string, data?: any) {
  console.log(JSON.stringify({ level, message, ...data, timestamp: new Date().toISOString() }));
}

async function callOutlayer(env: Env, method: string, params: any[]): Promise<any> {
  const response = await fetch(env.OUTLAYER_API_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Payment-Key': env.PAYMENT_KEY },
    body: JSON.stringify({ input: { id: 1, method, params } }),
  });
  
  if (!response.ok) {
    throw new Error(`OutLayer HTTP ${response.status}`);
  }
  
  const data = await response.json() as any;
  
  if (data.status !== 'completed' || !data.output) {
    throw new Error(data.error || 'OutLayer execution failed');
  }
  
  // output is already an object (not a string)
  if (data.output.error) {
    throw new Error(data.output.error);
  }
  
  return data.output.result;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const clientIp = request.headers.get('CF-Connecting-IP') || 'unknown';

    // Health check
    if (url.pathname === '/' || url.pathname === '/health') {
      return new Response(JSON.stringify({
        status: 'ok',
        version: '1.3.0',
        service: 'NEAR + Nostr Bunker',
        timestamp: new Date().toISOString(),
        endpoints: {
          websocket: `wss://${url.host}`,
          auth: `https://${url.host}/auth/{account_id}`,
          api: `https://${url.host}/api/{method}`
        }
      }), { headers: { 'Content-Type': 'application/json' } });
    }

    // API: Get public key
    if (url.pathname === '/api/get_public_key' && request.method === 'POST') {
      log('info', 'API: get_public_key', { ip: clientIp });
      try {
        const pubkey = await callOutlayer(env, 'get_public_key', []);
        log('info', 'Got pubkey', { pubkey: pubkey.slice(0, 16) + '...' });
        return new Response(JSON.stringify({ pubkey }), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        log('error', 'get_public_key failed', { error: String(e) });
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500,
          headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Get identity
    if (url.pathname === '/api/get_identity' && request.method === 'POST') {
      log('info', 'API: get_identity', { ip: clientIp });
      try {
        const identity = await callOutlayer(env, 'get_identity', []);
        log('info', 'Got identity', { near: identity.near_account });
        return new Response(JSON.stringify(identity), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        log('error', 'get_identity failed', { error: String(e) });
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500,
          headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Create identity proof (NIP-39)
    if (url.pathname === '/api/create_identity_proof' && request.method === 'POST') {
      log('info', 'API: create_identity_proof', { ip: clientIp });
      try {
        const proof = await callOutlayer(env, 'create_identity_proof', []);
        log('info', 'Created identity proof');
        return new Response(JSON.stringify(proof), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        log('error', 'create_identity_proof failed', { error: String(e) });
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500,
          headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Sign event
    if (url.pathname === '/api/sign_event' && request.method === 'POST') {
      log('info', 'API: sign_event', { ip: clientIp });
      try {
        const body = await request.json();
        const signedEvent = await callOutlayer(env, 'sign_event', [body]);
        log('info', 'Event signed', { eventId: signedEvent.id?.slice(0, 16) });
        return new Response(JSON.stringify(signedEvent), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        log('error', 'sign_event failed', { error: String(e) });
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500,
          headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Create session
    if (url.pathname === '/api/create_session' && request.method === 'POST') {
      log('info', 'API: create_session', { ip: clientIp });
      try {
        const session = await callOutlayer(env, 'create_session', []);
        log('info', 'Session created');
        return new Response(JSON.stringify(session), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        log('error', 'create_session failed', { error: String(e) });
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500,
          headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // Auth page
    if (url.pathname.startsWith('/auth/')) {
      const accountId = url.pathname.split('/')[2] || 'unknown';
      const html = `<!DOCTYPE html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Authorize - ${accountId}</title>
<style>body{font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh;background:linear-gradient(135deg,#667eea,#764ba2);margin:0}.box{background:#fff;border-radius:20px;padding:40px;max-width:500px;width:90%;text-align:center;box-shadow:0 20px 60px rgba(0,0,0,.3)}button{width:100%;padding:16px;background:#007AFF;color:#fff;border:none;border-radius:12px;font-size:18px;cursor:pointer;margin-top:20px}button:disabled{background:#ccc;cursor:not-allowed}button.secondary{background:#34C759;margin-top:10px}#logs{text-align:left;font-size:12px;color:#666;margin-top:20px;max-height:200px;overflow-y:auto;background:#f5f5f5;padding:10px;border-radius:8px}#identity{background:#f0f8ff;padding:15px;border-radius:8px;margin-top:15px;text-align:left;font-size:14px}</style></head>
<body><div class="box"><h1>🔐 Authorize Nostr</h1><p>Account: <b>${accountId}</b></p><button id="btn">Login with NEAR</button><button id="proofBtn" class="secondary" style="display:none">Publish Identity Proof</button><div id="identity" style="display:none"></div><p id="status"></p><div id="logs"></div></div></body>
<script type="module">
import { NearConnector } from "https://esm.run/@hot-labs/near-connect";

const status = document.getElementById('status');
const btn = document.getElementById('btn');
const proofBtn = document.getElementById('proofBtn');
const logs = document.getElementById('logs');
const identityDiv = document.getElementById('identity');

function log(msg) {
  const line = document.createElement('div');
  line.textContent = new Date().toLocaleTimeString() + ' - ' + msg;
  logs.appendChild(line);
}

log('Page loaded');

const connector = new NearConnector();

connector.on("wallet:signIn", async (t) => {
  const address = t.accounts[0].accountId;
  log('✓ Wallet: ' + address);
  
  if (address !== '${accountId}') {
    status.innerHTML = '<span style="color:red">Wrong account: ' + address + '</span>';
    btn.disabled = false;
    return;
  }
  
  log('→ Getting identity...');
  try {
    const res = await fetch('/api/get_identity', { method: 'POST' });
    const data = await res.json();
    log('← Identity loaded');
    
    if (data.error) throw new Error(data.error);
    
    // Show identity info
    identityDiv.innerHTML = '<b>Your Nostr Identity:</b><br>' +
      'npub: <code>' + data.nostr_npub + '</code><br>' +
      'NEAR: <code>' + data.near_account + '</code><br>' +
      '<b>Bunker URL:</b><br><code style="font-size:11px;word-break:break-all">' + data.bunker_url + '</code><br>' +
      '<a href="' + data.verification_url + '" target="_blank">Verify on NEAR Social →</a>';
    identityDiv.style.display = 'block';
    
    // Show proof button
    proofBtn.style.display = 'block';
    
    status.innerHTML = '<span style="color:green">✓ Authorized!</span>';
    btn.textContent = '✓ Done';
    
    log('✓ Ready');
  } catch (e) {
    log('✗ Error: ' + e.message);
    status.innerHTML = '<span style="color:red">Error: ' + e.message + '</span>';
    btn.disabled = false;
  }
});

btn.onclick = async () => {
  btn.disabled = true;
  btn.textContent = 'Connecting...';
  log('→ Opening wallet...');
  try {
    await connector.connect();
  } catch (e) {
    log('✗ Error: ' + e.message);
    status.innerHTML = '<span style="color:red">Error: ' + e.message + '</span>';
    btn.disabled = false;
    btn.textContent = 'Login with NEAR';
  }
};

proofBtn.onclick = async () => {
  proofBtn.disabled = true;
  proofBtn.textContent = 'Publishing...';
  log('→ Creating NIP-39 identity proof...');
  try {
    const res = await fetch('/api/create_identity_proof', { method: 'POST' });
    const proof = await res.json();
    log('← Proof created');
    
    // Sign the event via bunker
    log('→ Signing event...');
    const signRes = await fetch('/api/sign_event', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(proof)
    });
    const signed = await signRes.json();
    
    if (signed.error) throw new Error(signed.error);
    log('← Event signed');
    
    // Publish to relays
    log('→ Publishing to Nostr relays...');
    const relays = [
      'wss://nostr-relay-production.up.railway.app',
      'wss://relay.damus.io',
      'wss://nos.lol'
    ];
    
    let published = 0;
    for (const relay of relays) {
      try {
        const ws = new WebSocket(relay);
        await new Promise((resolve, reject) => {
          ws.onopen = () => {
            ws.send(JSON.stringify(['EVENT', signed]));
            log('  → Published to ' + relay);
            published++;
            setTimeout(() => { ws.close(); resolve(true); }, 500);
          };
          ws.onerror = () => resolve(false);
          setTimeout(() => { ws.close(); resolve(false); }, 2000);
        });
      } catch (e) {
        log('  ✗ Failed: ' + relay);
      }
    }
    
    status.innerHTML = '<span style="color:green">✓ Identity published to ' + published + ' relays!<br>Your NEAR ↔ Nostr link is now verifiable.</span>';
    proofBtn.textContent = '✓ Published';
    proofBtn.style.background = '#34C759';
    
    log('✓ Done - published to ' + published + ' relays');
  } catch (e) {
    log('✗ Error: ' + e.message);
    proofBtn.disabled = false;
    proofBtn.textContent = 'Publish Identity Proof';
  }
};
</script></html>`;
      return new Response(html, { headers: { 'Content-Type': 'text/html' } });
    }

    // WebSocket for NIP-46
    if (request.headers.get('Upgrade') === 'websocket') {
      const pair = new WebSocketPair();
      const [client, server] = Object.values(pair);
      server.accept();
      
      server.addEventListener('message', async (event: MessageEvent) => {
        try {
          const msg: NIP46Request = JSON.parse(event.data as string);
          const result = await callOutlayer(env, msg.method, msg.params);
          server.send(JSON.stringify({ id: msg.id, result, error: null }));
        } catch (e) {
          server.send(JSON.stringify({ id: null, result: null, error: String(e) }));
        }
      });
      
      return new Response(null, { status: 101, webSocket: client });
    }

    return new Response('Not Found', { status: 404 });
  },
};
