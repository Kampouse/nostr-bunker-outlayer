import { WebSocketPair } from '@cloudflare/workers-types';

export interface Env {
  PAYMENT_KEY: string;
  OUTLAYER_API_URL: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    // Upgrade to WebSocket
    if (request.headers.get('Upgrade') === 'websocket') {
      return this.handleWebSocket(request, env);
    }
    
    // Health check
    if (request.url === '/') {
      return new Response('NEAR + Nostr Bunker (OutLayer + Bridge)', {
        headers: { 'Content-Type': 'text/plain' },
      });
    }
    
    return new Response('Expected WebSocket', { status: 426 });
  }

  private async handleWebSocket(request: Request, env: Env): Promise<Response> {
    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    
    server.accept();
    
    server.addEventListener('message', async (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string);
        
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
        
        const result = await response.json();
        
        // Send back to client
        server.send(JSON.stringify(result));
      } catch (error) {
        server.send(JSON.stringify({
          id: 'error',
          error: (error as Error).message,
        }));
      }
    });
    
    server.addEventListener('close', () => {
      console.log('WebSocket closed');
    });
    
    return new Response(null, {
      status: 101,
      webSocket: client,
    });
  }
}
}
