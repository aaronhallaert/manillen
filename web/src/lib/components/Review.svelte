<script lang="ts">
  import Table from './Table.svelte';
  import ParTable from './ParTable.svelte';
  import {
    compareCards,
    moveLabel,
    parseCard,
    sameMove,
    type AnalysisReport,
    type Bid,
    type Card,
    type Decision,
    type GameRecord,
    type Label,
    type ParReport,
    type PlayerView,
    type Suit,
  } from '../engine/types';

  interface Props {
    record: GameRecord;
    analysis: AnalysisReport | null;
    pending: boolean;
    par?: ParReport | null;
    parPending?: boolean;
    viewAt: (index: number) => Promise<PlayerView | null>;
    onback: () => void;
  }

  let { record, analysis, pending, par = null, parPending = false, viewAt, onback }: Props = $props();

  const actualBid: Bid | null = $derived.by(() => {
    const m = record.moves.find((mv) => mv.type === 'bid');
    return m && m.type === 'bid' ? m.bid : null;
  });

  let selected: Decision | null = $state(null);
  let selectedView: PlayerView | null = $state(null);
  let loadingView = $state(false);

  const LABEL_TEXT: Record<Label, string> = {
    best: 'Best',
    good: 'Good',
    inaccuracy: 'Inaccuracy',
    mistake: 'Mistake',
    blunder: 'Blunder',
  };

  async function select(d: Decision) {
    selected = d;
    loadingView = true;
    selectedView = await viewAt(d.index);
    loadingView = false;
  }

  const decisions = $derived(analysis?.decisions ?? []);
  const selectedPos = $derived(selected ? decisions.findIndex((d) => d.index === selected?.index) : -1);

  function step(delta: number) {
    const next = decisions[selectedPos + delta];
    if (next) void select(next);
  }

  // Show the first decision as soon as the analysis arrives.
  $effect(() => {
    if (analysis && decisions.length > 0 && selected === null) void select(decisions[0]);
    if (!analysis) {
      selected = null;
      selectedView = null;
    }
  });

  const SUITS: Suit[] = ['C', 'D', 'H', 'S'];
  const SUIT_SYM: Record<Suit, string> = { C: '♣', D: '♦', H: '♥', S: '♠' };

  /** Ranks of `cards` per suit, high to low, for the counting panel. */
  function bySuit(cards: Card[]): { suit: Suit; ranks: string[] }[] {
    const sorted = [...cards].sort(compareCards).reverse();
    return SUITS.map((suit) => ({
      suit,
      ranks: sorted.filter((c) => parseCard(c).suit === suit).map((c) => parseCard(c).rank),
    }));
  }

  function fmt(v: number): string {
    return (Math.round(v * 10) / 10).toFixed(1);
  }

  function describe(d: Decision): string {
    if (d.phase === 'bidding') return 'Bid';
    if (d.phase === 'contra') return 'Contra';
    return `Trick ${d.trick + 1}`;
  }

  // ---- eval graph ----
  const W = 640;
  const H = 120;
  const PAD = { l: 28, r: 8, t: 8, b: 18 };

  const graph = $derived.by(() => {
    const pts = analysis?.timeline ?? [];
    if (pts.length === 0) return null;
    // The timeline only holds play decisions: expected card points, 0..60, 30 = break-even.
    const lo = 0;
    const hi = 60;
    const span = hi - lo;
    const x = (i: number) => PAD.l + (i / Math.max(1, pts.length - 1)) * (W - PAD.l - PAD.r);
    const y = (v: number) => PAD.t + (1 - (v - lo) / span) * (H - PAD.t - PAD.b);
    const mid = 30;
    const path = pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join(' ');
    const dots = pts.map((p, i) => {
      const d = analysis?.decisions.find((dd) => dd.index === p.index);
      return { x: x(i), y: y(p.value), label: d?.label ?? 'best', index: p.index, d, trick: d ? d.trick + 1 : i + 1 };
    });
    return { path, dots, midY: y(mid), lo, hi, mid, x, y };
  });
</script>

<div class="review">
  <header>
    <button type="button" class="btn small" onclick={onback}>‹ Back</button>
    <h2>Review · dealer {record.dealer === analysis?.player ? 'you' : 'opp'}</h2>
    {#if analysis}
      <div class="summary">
        {#each ['best', 'good', 'inaccuracy', 'mistake', 'blunder'] as const as lbl (lbl)}
          <span class="badge {lbl}">{analysis.summary[lbl]} {LABEL_TEXT[lbl]}</span>
        {/each}
        <span class="avg">avg loss {fmt(analysis.summary.avg_loss)}</span>
      </div>
    {/if}
  </header>

  {#if pending}
    <div class="progress" role="status">
      <div class="bar"></div>
      <span>Analysing your decisions…</span>
    </div>
  {:else if analysis}
    <div class="body">
      <section class="left">
        {#if selected}
          <div class="detail">
            <div class="detail-head">
              <button type="button" class="btn small" disabled={selectedPos <= 0} onclick={() => step(-1)} aria-label="previous decision">‹</button>
              <h3>{describe(selected)} · you played <strong>{moveLabel(selected.chosen)}</strong> <span class="badge {selected.label}">{LABEL_TEXT[selected.label]}</span></h3>
              <button type="button" class="btn small" disabled={selectedPos >= decisions.length - 1} onclick={() => step(1)} aria-label="next decision">›</button>
            </div>
            {#if !sameMove(selected.chosen, selected.best)}
              <p>Best was <strong>{moveLabel(selected.best)}</strong> ({fmt(selected.value_best)} vs {fmt(selected.value_chosen)} expected {selected.unit}).</p>
            {:else}
              <p>This was the best option ({fmt(selected.value_best)} expected {selected.unit}).</p>
            {/if}
            {#if selected.note}<p class="note">{selected.note}</p>{/if}
            {#if selected.reasons_best?.length}
              <ul class="reasons good">
                {#each selected.reasons_best as r (r)}<li>{r}</li>{/each}
              </ul>
            {/if}
            {#if selected.reasons_chosen?.length}
              <p class="why-label">What {moveLabel(selected.chosen)} did instead:</p>
              <ul class="reasons bad">
                {#each selected.reasons_chosen as r (r)}<li>{r}</li>{/each}
              </ul>
            {/if}
            {#if selected.samples}
              <p class="muted small">
                Values are average expected points over {selected.samples} possible layouts of the cards you could not see
                (opponent's hand and face-down cards), each played out perfectly by both sides.
              </p>
            {/if}
          </div>
          {#if selectedView}
            <div class="counting">
              <div>
                <span class="why-label">Played so far ({selectedView.played.length})</span>
                {#each bySuit(selectedView.played) as row (row.suit)}
                  <span class="suitrow" class:red={row.suit === 'D' || row.suit === 'H'}>
                    <span class="sym">{SUIT_SYM[row.suit]}</span>{row.ranks.length ? row.ranks.join(' ') : '–'}
                  </span>
                {/each}
              </div>
              <div>
                <span class="why-label">Not seen yet ({selectedView.unknown.length})</span>
                {#each bySuit(selectedView.unknown) as row (row.suit)}
                  <span class="suitrow" class:red={row.suit === 'D' || row.suit === 'H'}>
                    <span class="sym">{SUIT_SYM[row.suit]}</span>{row.ranks.length ? row.ranks.join(' ') : '–'}
                  </span>
                {/each}
              </div>
            </div>
          {/if}
          <div class="position" class:loading={loadingView}>
            {#if selectedView}
              <Table view={selectedView} readonly />
            {:else}
              <p class="muted">loading position…</p>
            {/if}
          </div>
          <ul class="alts">
            {#each [...selected.options].sort((a, b) => b.value - a.value) as opt (moveLabel(opt.move))}
              <li class:chosen={sameMove(opt.move, selected.chosen)}>
                <span>{moveLabel(opt.move)}</span>
                <span class="val">{fmt(opt.value)}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <ol class="moves" aria-label="decisions">
        {#each decisions as d (d.index)}
          <li>
            <button type="button" class="move" class:active={selected?.index === d.index} onclick={() => select(d)}>
              <span class="when">{describe(d)}</span>
              <span class="what">{moveLabel(d.chosen)}</span>
              <span class="badge {d.label}">{LABEL_TEXT[d.label]}</span>
              <span class="loss">{d.loss > 0 ? `−${fmt(d.loss)}` : '±0'}</span>
            </button>
          </li>
        {/each}
      </ol>
    </div>

    <details class="parbox">
      <summary>Par (perfect information){#if parPending} · computing…{/if}</summary>
      {#if par && analysis}
        <ParTable {par} me={analysis.player} {actualBid} />
      {:else if !parPending}
        <p class="muted">not available</p>
      {/if}
    </details>

    {#if graph}
      <svg class="graph" viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="expected points per trick">
        <line x1={PAD.l} x2={W - PAD.r} y1={graph.midY} y2={graph.midY} class="mid" />
        <text x={PAD.l - 4} y={graph.y(graph.hi) + 4} class="axis">{graph.hi}</text>
        <text x={PAD.l - 4} y={graph.midY + 4} class="axis">{graph.mid}</text>
        <text x={PAD.l - 4} y={graph.y(graph.lo) + 4} class="axis">{graph.lo}</text>
        <path d={graph.path} class="line" />
        {#each graph.dots as dot (dot.index)}
          <text x={dot.x} y={H - 4} class="axis tick">{dot.trick}</text>
          <circle
            cx={dot.x}
            cy={dot.y}
            r={selected?.index === dot.index ? 6 : 4}
            class="dot {dot.label}"
            role="button"
            tabindex="0"
            onclick={() => dot.d && select(dot.d)}
            onkeydown={(e) => e.key === 'Enter' && dot.d && select(dot.d)}
          />
        {/each}
      </svg>
    {/if}
  {:else}
    <p class="muted">No analysis available.</p>
  {/if}
</div>

<style>
  .review {
    padding: 8px 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    max-width: 1100px;
    margin: 0 auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 12px;
    align-items: start;
  }
  .left {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }
  .moves {
    min-width: 0;
  }
  .move > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @media (max-width: 760px) {
    .body {
      grid-template-columns: 1fr;
    }
    .moves {
      max-height: 40vh;
    }
  }
  header h2 {
    font-size: 1.05rem;
    margin: 0;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    padding: 24px 0;
    color: var(--muted);
  }
  .bar {
    width: 80%;
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(90deg, var(--accent) 0 40%, var(--panel) 40% 100%);
    background-size: 200% 100%;
    animation: slide 1.2s linear infinite;
  }
  @keyframes slide {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: 0.8rem;
    margin-left: auto;
  }
  .avg {
    color: var(--muted);
  }
  .badge {
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 0.72rem;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
  }
  .badge.best {
    background: var(--c-best);
  }
  .badge.good {
    background: var(--c-good);
  }
  .badge.inaccuracy {
    background: var(--c-inaccuracy);
    color: #222;
  }
  .badge.mistake {
    background: var(--c-mistake);
  }
  .badge.blunder {
    background: var(--c-blunder);
  }
  .parbox {
    background: var(--panel);
    border-radius: 12px;
    padding: 6px 12px;
  }
  .parbox summary {
    cursor: pointer;
    font-weight: 600;
    font-size: 0.9rem;
    min-height: 36px;
    display: flex;
    align-items: center;
  }
  .graph {
    width: 100%;
    height: 140px;
    background: var(--panel);
    border-radius: 12px;
  }
  .graph .tick {
    text-anchor: middle;
    font-size: 8px;
  }
  .graph .mid {
    stroke: var(--muted);
    stroke-dasharray: 3 3;
    stroke-width: 1;
  }
  .graph .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }
  .graph .axis {
    font-size: 9px;
    fill: var(--muted);
    text-anchor: end;
  }
  .dot {
    cursor: pointer;
    stroke: var(--panel);
    stroke-width: 1.5;
  }
  .dot.best {
    fill: var(--c-best);
  }
  .dot.good {
    fill: var(--c-good);
  }
  .dot.inaccuracy {
    fill: var(--c-inaccuracy);
  }
  .dot.mistake {
    fill: var(--c-mistake);
  }
  .dot.blunder {
    fill: var(--c-blunder);
  }
  .moves {
    list-style: none;
    margin: 0;
    padding: 0 4px 0 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: calc(100dvh - 300px);
    min-height: 200px;
    overflow-y: auto;
  }
  .move {
    width: 100%;
    min-height: 38px;
    display: grid;
    grid-template-columns: 5.5rem 1fr auto 3.5rem;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: var(--panel);
    color: var(--fg);
    font: inherit;
    text-align: left;
  }
  .move.active {
    border-color: var(--accent);
  }
  .when {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .what {
    font-weight: 700;
  }
  .loss {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .detail {
    background: var(--panel);
    border-radius: 12px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .detail-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .detail-head h3 {
    flex: 1;
    min-width: 0;
  }
  .detail h3 {
    margin: 0;
    font-size: 0.95rem;
  }
  .detail p {
    margin: 0;
    font-size: 0.9rem;
  }
  .note {
    font-style: italic;
    color: var(--muted);
  }
  .reasons {
    margin: 0;
    padding-left: 18px;
    font-size: 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .reasons.good li::marker {
    color: var(--c-best);
  }
  .reasons.bad li::marker {
    color: var(--c-mistake);
  }
  .why-label {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 4px 0 0;
    display: block;
  }
  .small {
    font-size: 0.75rem;
  }
  .counting {
    background: var(--panel);
    border-radius: 12px;
    padding: 8px 12px;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 16px;
    font-size: 0.85rem;
  }
  .counting > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .suitrow {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .suitrow .sym {
    display: inline-block;
    width: 1.2em;
  }
  .suitrow.red .sym {
    color: var(--card-red);
  }
  .alts {
    list-style: none;
    margin: 0;
    padding: 8px 10px;
    background: var(--panel);
    border-radius: 12px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 4px 12px;
    font-size: 0.85rem;
  }
  .alts li {
    display: flex;
    justify-content: space-between;
    padding: 4px 8px;
    border-radius: 6px;
  }
  .alts li.chosen {
    background: var(--felt-dark);
    color: #f4f1ea;
    font-weight: 700;
  }
  .val {
    font-variant-numeric: tabular-nums;
  }
  .position {
    border-radius: 12px;
    background: var(--felt);
    color: #f4f1ea;
    --fg: #f4f1ea;
    --muted: #cfd8cf;
    --panel: #10301f;
    --line: #2b5a40;
    --card-w: 44px;
    --card-w-small: 34px;
    overflow: hidden;
    min-height: 300px;
  }
  .position.loading {
    opacity: 0.6;
  }
  .muted {
    color: var(--muted);
  }
</style>
