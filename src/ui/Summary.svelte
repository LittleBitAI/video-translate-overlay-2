<script lang="ts" module>
  export type Block = { kind: 'heading' | 'paragraph'; text: string } | { kind: 'list'; items: string[] };

  const unescape = (s: string) => s.replace(/\\(.)/g, '$1');

  /** P19's Markdown as blocks of text: the template's `## ` headings, `- ` lists and paragraphs, never HTML. */
  export function blocks(markdown: string): Block[] {
    const out: Block[] = [];
    for (const line of markdown.split('\n')) {
      if (!line.trim()) continue;
      const last = out.at(-1);
      if (line.startsWith('## ')) out.push({ kind: 'heading', text: unescape(line.slice(3)) });
      else if (line.startsWith('- ') && last?.kind === 'list') last.items.push(unescape(line.slice(2)));
      else if (line.startsWith('- ')) out.push({ kind: 'list', items: [unescape(line.slice(2))] });
      else out.push({ kind: 'paragraph', text: unescape(line) });
    }
    return out;
  }
</script>

<script lang="ts">
  // A ready video's summary (P19). The request belongs to the account the
  // screen's API call captures; a reply that comes after this view is gone or
  // has asked again is dropped here, and the backend has stored it anyway, so
  // opening the video again shows it.
  import { onDestroy, onMount } from 'svelte';
  import { request, won } from './api';

  let { videoKey }: { videoKey: string } = $props();

  let look = $state<'checking' | 'none' | 'loading' | 'shown'>('checking');
  let markdown = $state('');
  let cost = $state(0);
  let error = $state('');
  /** What the failed press asked for, so 다시 시도 asks the same. */
  let lastAgain = false;
  let generation = 0;
  let gone = false;
  const path = $derived(`/videos/${encodeURIComponent(videoKey)}/summary`);
  const shown = $derived(blocks(markdown));

  async function read() {
    const mine = ++generation;
    const r = await request('GET', path);
    if (gone || mine !== generation) return;
    if (r.status === 200 && typeof r.body?.markdown === 'string') {
      markdown = r.body.markdown;
      cost = r.body.cost_krw;
      look = 'shown';
    } else {
      // Not made yet, or not readable now: 요약 asks, and answers a stored one without a Gemini call.
      look = 'none';
    }
  }

  async function summarize(again: boolean) {
    const mine = ++generation;
    const before = look;
    lastAgain = again;
    error = '';
    look = 'loading';
    const r = await request('POST', path, again ? { again: true } : {});
    if (gone || mine !== generation) return;
    if (r.status === 200 && typeof r.body?.markdown === 'string') {
      markdown = r.body.markdown;
      cost = r.body.cost_krw;
      look = 'shown';
      return;
    }
    look = before === 'shown' ? 'shown' : 'none';
    error =
      r.status === 428
        ? '개인키가 없어 요약하지 않았습니다. 설정에서 Gemini 개인키를 넣어 주세요.'
        : (r.body?.message ?? `요약하지 못했습니다 (${r.status}).`);
    // A refused reply was still billed: show the stored summary with what it has cost now.
    const stored = await request('GET', path);
    if (gone || mine !== generation) return;
    if (stored.status === 200 && typeof stored.body?.markdown === 'string') {
      markdown = stored.body.markdown;
      cost = stored.body.cost_krw;
      look = 'shown';
    }
  }

  onMount(read);
  onDestroy(() => {
    gone = true;
    generation++;
  });

  const quiet = 'min-h-11 shrink-0 rounded-lg border border-ink/40 px-4 py-2 text-base';
</script>

<section data-feature="video-summary" aria-labelledby="summary-heading" aria-busy={look === 'loading' || look === 'checking'} class="mt-4 max-w-3xl">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <h2 id="summary-heading" class="text-lg font-semibold">강의 요약</h2>
    {#if look === 'none' || look === 'loading'}
      <button
        type="button"
        class="min-h-11 shrink-0 rounded-lg bg-primary px-4 py-2 text-base font-semibold text-surface disabled:opacity-60"
        disabled={look === 'loading'}
        onclick={() => summarize(false)}>{look === 'loading' ? '요약하는 중' : '요약'}</button
      >
    {:else if look === 'shown'}
      <button type="button" class={quiet} onclick={() => summarize(true)}>다시 요약</button>
    {/if}
  </div>

  {#if look === 'none' && !error}
    <p class="mt-2 text-sm text-ink/70">제목과 원문 자막으로 Gemini가 요약합니다. 몇 원 정도의 비용이 들고, 영상 비용에 더해집니다.</p>
  {:else if look === 'loading'}
    <p role="status" class="mt-2 text-sm text-ink/70">요약하는 중입니다. 화면을 떠나도 요약은 저장되고, 영상을 다시 열면 보입니다.</p>
  {/if}

  {#if error}
    <div role="alert" class="mt-2 rounded-lg border border-accent p-2 text-sm">
      <p class="text-accent">{error}</p>
      <button type="button" class="{quiet} mt-2" onclick={() => summarize(lastAgain)}>다시 시도</button>
    </div>
  {/if}

  {#if look === 'shown'}
    <article class="mt-2 text-base">
      {#each shown as block, i (i)}
        {#if block.kind === 'heading'}
          <h3 class="mt-4 text-base font-semibold first:mt-0">{block.text}</h3>
        {:else if block.kind === 'list'}
          <ul class="mt-1 list-disc space-y-1 pl-6 text-ink/90">
            {#each block.items as item, j (j)}<li>{item}</li>{/each}
          </ul>
        {:else}
          <p class="mt-1 text-ink/90">{block.text}</p>
        {/if}
      {/each}
    </article>
    <p class="mt-4 text-sm text-ink/60">요약 비용 {won(cost)} · 영상 비용에 포함</p>
  {/if}
</section>
