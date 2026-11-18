import { expect, test } from 'vitest';
import './fake';

const OtherContextChannel = globalThis.BroadcastChannel;

test('broadcast fixtures share peer messages without receiving other test contexts', async () => {
  const sender = new BroadcastChannel('vto-library');
  const receiver = new BroadcastChannel('vto-library');
  const unrelated = new OtherContextChannel('vto-library');
  const messages: unknown[] = [];
  try {
    const delivered = new Promise<void>(resolve => {
      receiver.onmessage = event => {
        messages.push(event.data);
        if (event.data === 'peer') resolve();
      };
    });
    unrelated.postMessage('other context');
    sender.postMessage('peer');
    await delivered;
    expect(messages).toEqual(['peer']);
  } finally {
    sender.close(); receiver.close(); unrelated.close();
  }
});
