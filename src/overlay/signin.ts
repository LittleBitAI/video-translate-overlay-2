// On Google's sign-in pages (the YouTube account screen P1 opens first): a
// skip button that goes on to the video, and the same step taken by itself
// when Google refuses to sign in inside an app (`disallowed_useragent`, "this
// browser or app may not be secure"), so the first open never ends there
// (F2, R22). It never reads the page's form: sign-in stays Google's (R23).

/** Where P1 sends the account screen on to: its `continue`, kept for the pages after it. */
const KEY = 'vto-continue';

const REFUSED_URL = [/\/signin\/rejected/i, /\/oauth\/error/i, /disallowed_useragent/i, /\/CookieMismatch/i];
const REFUSED_TEXT = [
  'disallowed_useragent',
  'This browser or app may not be secure',
  '브라우저 또는 앱이 안전하지 않을 수 있습니다',
  '이 브라우저 또는 앱은 안전하지 않을 수 있습니다',
];

/**
 * A YouTube address, always over https, or nothing: the button never leads
 * anywhere else. P1 takes http addresses too, and YouTube serves only https,
 * so an http one is the same video and goes there over https.
 */
function youtubeTarget(raw: string | null): string | null {
  if (!raw) return null;
  try {
    const u = new URL(raw);
    const youtube = /(^|\.)youtube\.com$/.test(u.hostname) || u.hostname === 'youtu.be';
    if (!youtube || (u.protocol !== 'https:' && u.protocol !== 'http:')) return null;
    u.protocol = 'https:';
    u.port = '';
    return u.href;
  } catch {
    return null;
  }
}

export function refused(href: string, text: string): boolean {
  return REFUSED_URL.some((r) => r.test(href)) || REFUSED_TEXT.some((t) => text.includes(t));
}

export interface SignInOptions {
  store: Storage;
  /** Opens the video (the page's own navigation in the app). */
  go: (url: string) => void;
  /** The page's address and text now (read once a second: Google's pages change both without a reload). */
  href?: () => string;
  text?: () => string;
}

export function signInHelp({
  store,
  go,
  href = () => location.href,
  text = () => document.body?.innerText ?? '',
}: SignInOptions): { destroy(): void } {
  let target = youtubeTarget(new URL(href()).searchParams.get('continue'));
  if (target) store.setItem(KEY, target);
  else target = youtubeTarget(store.getItem(KEY));
  if (!target) return { destroy() {} };
  const to = target;

  const host = document.createElement('div');
  host.setAttribute('data-vto', 'sign-in');
  host.style.setProperty('all', 'initial', 'important');
  const shadow = host.attachShadow({ mode: 'open' });
  const style = document.createElement('style');
  style.textContent = `
:host { all: initial; }
.bar { position: fixed; left: 0; right: 0; bottom: 16px; z-index: 2147483647; display: flex; justify-content: center;
  padding: 0 16px; box-sizing: border-box; font-family: system-ui, "Noto Sans KR", sans-serif; pointer-events: none; }
button { pointer-events: auto; font: inherit; font-size: 14px; padding: 8px 14px; border: 0; border-radius: 18px;
  background: #424242; color: #fff; cursor: pointer; }
`;
  const bar = document.createElement('div');
  bar.className = 'bar';
  const button = document.createElement('button');
  button.type = 'button';
  button.dataset.vto = 'skip-sign-in';
  button.textContent = '로그인 건너뛰고 영상 보기';
  bar.append(button);
  shadow.append(style, bar);
  let gone = false;
  const leave = () => {
    if (gone) return;
    gone = true;
    store.removeItem(KEY);
    go(to);
  };
  button.addEventListener('click', leave);
  (document.body ?? document.documentElement).append(host);

  const watch = () => {
    if (refused(href(), text())) leave();
  };
  const timer = setInterval(watch, 1_000);
  watch();
  return {
    destroy() {
      clearInterval(timer);
      host.remove();
    },
  };
}
