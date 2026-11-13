import { expect, onTestFinished, test } from 'vitest';

test('video detection and sign-in controls work under YouTube-style Trusted Types enforcement', async () => {
  const frame = document.createElement('iframe');
  frame.src = '/tests/overlay/fixtures/trusted-types.html';
  frame.style.cssText = 'width:700px;height:600px;border:0';
  document.body.append(frame);
  onTestFinished(() => frame.remove());
  const probe = () => (frame.contentWindow as any)?.probe;
  await expect.poll(() => probe()?.ready, { timeout: 10_000 }).toBe(true);

  const doc = frame.contentDocument!;
  expect(() => { doc.createElement('div').innerHTML = '<span>blocked</span>'; }).toThrow();
  expect(probe().errors).toEqual([]);
  const control = (host: string, part: string) => doc.querySelector(host)?.shadowRoot?.querySelector(part) as HTMLButtonElement;
  const detect = control('[data-vto="overlay"]', '[data-vto="detect"]');
  expect(detect.textContent).toBe('영상감지');
  detect.click();
  await expect.poll(() => probe().calls.some((c: any) => c.method === 'POST' && c.path === '/jobs')).toBe(true);
  await expect.poll(() => control('[data-vto="overlay"]', '[data-vto="status"]').textContent).toContain('Gemini API 키');
  control('[data-vto="sign-in"]', '[data-vto="skip-sign-in"]').click();
  expect(probe().opened).toEqual(['https://www.youtube.com/watch?v=abcDEF12345']);
});
