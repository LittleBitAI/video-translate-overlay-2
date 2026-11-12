// The screens by path: S1 at `/`, S2 at `/preferences`. Links move by
// history.pushState; a fresh load at either path opens that screen.

let path = $state(location.pathname);

addEventListener('popstate', () => {
  path = location.pathname;
});

export const router = {
  get path() {
    return path;
  },
};

export function go(to: string) {
  if (to !== location.pathname) history.pushState(null, '', to);
  path = to;
  scrollTo(0, 0);
}
