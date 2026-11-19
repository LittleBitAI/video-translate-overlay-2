/** The widths of the lines `el`'s text is laid out on, top to bottom. */
export function lineWidths(el: Element): number[] {
  const range = document.createRange();
  range.selectNodeContents(el);
  const lines = new Map<number, { left: number; right: number }>();
  for (const r of range.getClientRects()) {
    if (r.width < 1) continue;
    const top = [...lines.keys()].find((t) => Math.abs(t - r.top) < r.height / 2) ?? r.top;
    const line = lines.get(top) ?? { left: r.left, right: r.right };
    lines.set(top, { left: Math.min(line.left, r.left), right: Math.max(line.right, r.right) });
  }
  return [...lines.entries()].sort((a, b) => a[0] - b[0]).map(([, l]) => l.right - l.left);
}

/** A long cue (49 characters): one sentence part at the cap, two lines on a phone. */
export const LONG_CUE = '학습률이 너무 높으면 손실이 크게 흔들리며 한 값으로 수렴하지 못하므로 조금씩 낮춥니다.';
