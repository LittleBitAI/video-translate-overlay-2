/** The visible image, excluding letterboxing and the part outside the viewport. */
export function videoArea(video: HTMLVideoElement): DOMRect {
  const box = video.getBoundingClientRect();
  const style = getComputedStyle(video);
  let { left, top, width, height } = box;
  if (video.videoWidth && video.videoHeight && ['contain', 'scale-down'].includes(style.objectFit)) {
    const scale = Math.min(width / video.videoWidth, height / video.videoHeight, style.objectFit === 'scale-down' ? 1 : Infinity);
    const imageWidth = video.videoWidth * scale;
    const imageHeight = video.videoHeight * scale;
    const position = style.objectPosition.split(' ');
    const offset = (value: string, space: number) => value.endsWith('%') ? space * parseFloat(value) / 100 : parseFloat(value);
    left += offset(position[0], width - imageWidth);
    top += offset(position[1], height - imageHeight);
    width = imageWidth;
    height = imageHeight;
  }
  const right = Math.min(innerWidth, left + width);
  const bottom = Math.min(innerHeight, top + height);
  left = Math.max(0, left);
  top = Math.max(0, top);
  return new DOMRect(left, top, Math.max(0, right - left), Math.max(0, bottom - top));
}

// Width sets the reading size; height limits it in wide, shallow players.
export const SUBTITLE_SIZE = 'max(14px, min(calc(var(--w) * 0.03), calc(var(--h) * 0.055)))';
