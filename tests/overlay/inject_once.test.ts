import { expect, onTestFinished, test } from 'vitest';
import { startPage } from '../../src/overlay/page';

test('repeated injection shares one subtitle cache and a stopped page can start again', async () => {
  const video = document.createElement('video');
  video.style.cssText = 'display:block;width:640px;height:360px';
  Object.defineProperty(video, 'duration', { value: 900 });
  document.body.append(video);
  onTestFinished(() => video.remove());
  const request = async () => ({ status: 404, body: {} });
  const first = startPage({ top: null, request });
  onTestFinished(() => first.stop());
  const second = startPage({ top: null, request });
  expect(second).toBe(first);
  await expect.poll(() => document.querySelectorAll('[data-vto=overlay]').length).toBe(1);
  first.stop();
  expect(document.querySelectorAll('[data-vto=overlay]')).toHaveLength(0);
  const third = startPage({ top: null, request });
  onTestFinished(() => third.stop());
  expect(third).not.toBe(first);
  await expect.poll(() => document.querySelectorAll('[data-vto=overlay]').length).toBe(1);
});
