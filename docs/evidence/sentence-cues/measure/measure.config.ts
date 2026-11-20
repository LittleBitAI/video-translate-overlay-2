// `npx vitest run -c docs/evidence/sentence-cues/measure/measure.config.ts`: the Watch screen's
// browser setup (vitest.ui.config.ts) with only the rendered-line measurement.
import ui from '../../../../vitest.ui.config';

export default { ...ui, test: { ...ui.test, include: ['docs/evidence/sentence-cues/measure/measure.test.svelte.ts'], testTimeout: 600_000 } };
