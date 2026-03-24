/**
 * NEAR + Nostr Bunker - Cloudflare Worker Bridge
 * Multi-user: Any NEAR account can derive their Nostr identity
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
  
  if (data.output.error) {
    throw new Error(data.output.error);
  }
  
  return data.output.result;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const clientIp = request.headers.get('CF-Connecting-IP') || 'unknown';

    // WebSocket check MUST come first
    if (request.headers.get('Upgrade')?.toLowerCase() === 'websocket') {
      const pair = new WebSocketPair();
      const [client, server] = Object.values(pair);
      server.accept();
      
      log('info', 'WebSocket connected', { ip: clientIp });
      
      server.addEventListener('message', async (event: MessageEvent) => {
        try {
          const msg: NIP46Request = JSON.parse(event.data as string);
          log('info', 'Request', { method: msg.method, ip: clientIp });
          const result = await callOutlayer(env, msg.method, msg.params);
          server.send(JSON.stringify({ id: msg.id, result, error: null }));
        } catch (e) {
          log('error', 'Request failed', { error: String(e), ip: clientIp });
          server.send(JSON.stringify({ id: null, result: null, error: String(e) }));
        }
      });
      
      return new Response(null, { status: 101, webSocket: client });
    }

    // Health check
    if (url.pathname === '/' || url.pathname === '/health') {
      return new Response(JSON.stringify({
        status: 'ok',
        version: '3.0.0',
        service: 'NEAR + Nostr Bunker (Multi-User)',
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
      const body = await request.json() as any;
      const accountId = body.account_id || 'kampouse.near';
      try {
        const pubkey = await callOutlayer(env, 'get_public_key', [accountId]);
        return new Response(JSON.stringify({ pubkey }), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500, headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Get identity
    if (url.pathname === '/api/get_identity' && request.method === 'POST') {
      const body = await request.json() as any;
      const accountId = body.account_id || 'kampouse.near';
      try {
        const identity = await callOutlayer(env, 'get_identity', [accountId]);
        return new Response(JSON.stringify(identity), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500, headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Get private key
    if (url.pathname === '/api/get_private_key' && request.method === 'POST') {
      const body = await request.json() as any;
      const accountId = body.account_id || 'kampouse.near';
      log('info', 'Private key requested', { account_id: accountId, ip: clientIp });
      try {
        const result = await callOutlayer(env, 'get_private_key', [accountId]);
        return new Response(JSON.stringify(result), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500, headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Create identity proof
    if (url.pathname === '/api/create_identity_proof' && request.method === 'POST') {
      const body = await request.json() as any;
      const accountId = body.account_id || 'kampouse.near';
      try {
        const proof = await callOutlayer(env, 'create_identity_proof', [accountId]);
        return new Response(JSON.stringify(proof), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500, headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // API: Sign event
    if (url.pathname === '/api/sign_event' && request.method === 'POST') {
      const body = await request.json() as any;
      const accountId = body.account_id || 'kampouse.near';
      const event = body.event || body;
      try {
        const signedEvent = await callOutlayer(env, 'sign_event', [accountId, event]);
        return new Response(JSON.stringify(signedEvent), { 
          headers: { 'Content-Type': 'application/json' } 
        });
      } catch (e) {
        return new Response(JSON.stringify({ error: String(e) }), { 
          status: 500, headers: { 'Content-Type': 'application/json' } 
        });
      }
    }

    // Auth page
    if (url.pathname.startsWith('/auth/')) {
      const accountId = url.pathname.split('/')[2] || 'unknown';
      const html = `<!DOCTYPE html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Authorize - ${accountId}</title>
<style>body{font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh;background:linear-gradient(135deg,#667eea,#764ba2);margin:0}.box{background:#fff;border-radius:20px;padding:40px;max-width:500px;width:90%;text-align:center;box-shadow:0 20px 60px rgba(0,0,0,.3)}button{width:100%;padding:16px;background:#007AFF;color:#fff;border:none;border-radius:12px;font-size:18px;cursor:pointer;margin-top:20px}button:disabled{background:#ccc;cursor:not-allowed}button.secondary{background:#34C759;margin-top:10px}#logs{text-align:left;font-size:12px;color:#666;margin-top:20px;max-height:200px;overflow-y:auto;background:#f5f5f5;padding:10px;border-radius:8px}</style></head>
<body><div class="box"><h1>🔐 Authorize Nostr</h1><p>Account: <b>${accountId}</b></p><button id="btn">Login with NEAR</button><button id="proofBtn" class="secondary" style="display:none">Publish Identity Proof</button><div id="identity" style="display:none"></div><div id="keySection" style="display:none;margin-top:20px;padding-top:20px;border-top:1px solid #ddd"><h3>🔑 Export Private Key</h3><p style="font-size:13px;color:#666">For <a href="https://snort.social" target="_blank">snort.social</a> or other clients</p><button id="showKeyBtn" style="background:#FF9500">Show Private Key (⚠️)</button><div id="keyWarning" style="display:none;background:#fff3cd;padding:15px;border-radius:8px;margin:10px 0"><b>⚠️ WARNING</b><br><br>Your private key gives FULL access to your Nostr identity.<br>Never share it or enter it on untrusted websites.<br><br><button id="confirmShowKey" style="background:#dc3545">I understand - Show Key</button></div><div id="keyDisplay" style="display:none"></div></div><p id="status"></p><div id="logs"></div></div></body>
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
    const res = await fetch('/api/get_identity', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ account_id: address })
    });
    const data = await res.json();
    log('← Identity loaded');
    
    if (data.error) throw new Error(data.error);
    
    identityDiv.innerHTML = '<b>Your Nostr Identity:</b><br>' +
      'npub: <code>' + data.nostr_npub + '</code><br>' +
      'NEAR: <code>' + data.near_account + '</code><br>' +
      '<b>Bunker URL:</b><br><code style="font-size:11px;word-break:break-all">' + data.bunker_url + '</code><br>' +
      '<a href="' + data.verification_url + '" target="_blank">Verify on NEAR Social →</a>';
    identityDiv.style.display = 'block';
    
    proofBtn.style.display = 'block';
    document.getElementById('keySection').style.display = 'block';
    
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
  log('→ Creating identity proof...');
  try {
    const res = await fetch('/api/create_identity_proof', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ account_id: '${accountId}' })
    });
    const proof = await res.json();
    log('← Proof created');
    
    const signRes = await fetch('/api/sign_event', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ account_id: '${accountId}', event: proof })
    });
    const signed = await signRes.json();
    
    if (signed.error) throw new Error(signed.error);
    log('← Event signed');
    
    const relays = ['wss://nostr-relay-production.up.railway.app', 'wss://relay.damus.io'];
    let published = 0;
    for (const relay of relays) {
      try {
        const ws = new WebSocket(relay);
        await new Promise((resolve) => {
          ws.onopen = () => {
            ws.send(JSON.stringify(['EVENT', signed]));
            log('  → Published to ' + relay);
            published++;
            setTimeout(() => { ws.close(); resolve(true); }, 500);
          };
          setTimeout(() => { ws.close(); resolve(false); }, 2000);
        });
      } catch (e) {}
    }
    
    status.innerHTML = '<span style="color:green">✓ Published to ' + published + ' relays!</span>';
    proofBtn.textContent = '✓ Published';
    proofBtn.style.background = '#34C759';
  } catch (e) {
    log('✗ Error: ' + e.message);
    proofBtn.disabled = false;
    proofBtn.textContent = 'Publish Identity Proof';
  }
};

const showKeyBtn = document.getElementById('showKeyBtn');
const keyWarning = document.getElementById('keyWarning');
const keyDisplay = document.getElementById('keyDisplay');

showKeyBtn.onclick = () => {
  keyWarning.style.display = 'block';
  showKeyBtn.style.display = 'none';
};

document.getElementById('confirmShowKey').onclick = async () => {
  keyWarning.style.display = 'none';
  log('→ Getting private key...');
  
  try {
    const res = await fetch('/api/get_private_key', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ account_id: '${accountId}' })
    });
    const data = await res.json();
    
    if (data.error) throw new Error(data.error);
    log('✓ Private key retrieved');
    
    keyDisplay.innerHTML = 
      '<div style="background:#fff3cd;padding:15px;border-radius:8px;margin:10px 0;border:2px solid #ffc107">' +
      '<b>⚠️ PRIVATE KEY - NEVER SHARE!</b><br><br>' +
      '<b>nsec:</b><br><code style="word-break:break-all;font-size:14px;background:#fff;padding:5px;display:block">' + data.nsec + '</code><br>' +
      '<b>Hex:</b><br><code style="word-break:break-all;font-size:12px;background:#fff;padding:5px;display:block">' + data.private_key + '</code><br>' +
      '<b style="color:red">⚠️ Anyone with this key can sign as you!</b>' +
      '</div>';
    keyDisplay.style.display = 'block';
  } catch (e) {
    log('✗ Error: ' + e.message);
    keyDisplay.innerHTML = '<span style="color:red">Error: ' + e.message + '</span>';
    keyDisplay.style.display = 'block';
  }
};
</script></html>`;
      return new Response(html, { headers: { 'Content-Type': 'text/html' } });
    }

    return new Response('Not Found', { status: 404 });
  },
};
