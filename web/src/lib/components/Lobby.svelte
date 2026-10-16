<script lang="ts">
  import { parseSeed, type Seed } from '../engine/client';
  import type { GameRecord, Level, Player, Settings } from '../engine/types';

  interface Props {
    settings: Settings;
    engineKind: 'wasm' | 'mock' | null;
    canContinue: boolean;
    lastRecord: GameRecord | null;
    onsettings: (patch: Partial<Settings>) => void;
    onstart: (seed?: Seed) => void;
    oncontinue: () => void;
    onreview: (record: GameRecord, player: Player) => void;
  }

  let { settings, engineKind, canContinue, lastRecord, onsettings, onstart, oncontinue, onreview }: Props = $props();

  let seedText = $state('');
  let recordText = $state('');
  let recordError: string | null = $state(null);

  const LEVELS: { id: Level; name: string; hint: string }[] = [
    { id: 'easy', name: 'Easy', hint: 'plays by simple rules, forgets earlier tricks' },
    { id: 'medium', name: 'Medium', hint: 'uses strategy, counts the big cards' },
    { id: 'hard', name: 'Hard', hint: 'full search, perfect counting' },
  ];

  function start() {
    const seed = parseSeed(seedText);
    onstart(seed ?? undefined);
  }

  function parseRecord(text: string): GameRecord | null {
    try {
      const rec = JSON.parse(text) as GameRecord;
      if (!rec || !Array.isArray(rec.moves) || !rec.layout) throw new Error('not a game record');
      return rec;
    } catch (e) {
      recordError = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  function reviewPasted() {
    recordError = null;
    const rec = parseRecord(recordText);
    if (rec) onreview(rec, settings.human);
  }

  async function reviewFile(ev: globalThis.Event) {
    recordError = null;
    const input = ev.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const rec = parseRecord(await file.text());
    if (rec) onreview(rec, settings.human);
    input.value = '';
  }
</script>

<div class="lobby">
  <h1>Manillen</h1>
  <p class="sub">Two-player manillen against the engine. Engine: <strong>{engineKind ?? '…'}</strong></p>

  <section class="panel">
    <h2>New match</h2>
    <div class="levels">
      {#each LEVELS as lvl (lvl.id)}
        <button type="button" class="level" class:active={settings.level === lvl.id} onclick={() => onsettings({ level: lvl.id })}>
          <span class="lname">{lvl.name}</span>
          <span class="lhint">{lvl.hint}</span>
        </button>
      {/each}
    </div>
    <label class="field">
      <span>Play to</span>
      <input type="number" min="1" step="1" value={settings.target} onchange={(e) => onsettings({ target: Math.max(1, Number(e.currentTarget.value) || 101) })} />
    </label>
    <label class="field">
      <span>Your seat</span>
      <select value={settings.human} onchange={(e) => onsettings({ human: Number(e.currentTarget.value) as Player })}>
        <option value={0}>Seat 0 (you deal second)</option>
        <option value={1}>Seat 1 (you deal first)</option>
      </select>
    </label>
    <label class="field check" title="Hindsight: computed with every card visible and both sides playing perfectly.">
      <span>Show par before playing<br /><small>hindsight: reveals what perfect play would score</small></span>
      <input type="checkbox" checked={settings.show_par ?? false} onchange={(e) => onsettings({ show_par: e.currentTarget.checked })} />
    </label>
    <label class="field">
      <span>Seed (optional)</span>
      <input type="text" placeholder="random" bind:value={seedText} inputmode="numeric" />
    </label>
    <div class="actions">
      <button type="button" class="btn primary" onclick={start}>Start match</button>
      {#if canContinue}
        <button type="button" class="btn" onclick={oncontinue}>Continue</button>
      {/if}
    </div>
  </section>

  <section class="panel">
    <h2>Review a deal</h2>
    {#if lastRecord}
      <button type="button" class="btn" onclick={() => onreview(lastRecord, settings.human)}>Review last deal</button>
    {/if}
    <label class="field">
      <span>Open record file</span>
      <input type="file" accept="application/json,.json" onchange={reviewFile} />
    </label>
    <label class="field col">
      <span>…or paste record JSON</span>
      <textarea rows="3" bind:value={recordText} placeholder={'{"version":1, ...}'}></textarea>
    </label>
    <button type="button" class="btn" disabled={!recordText.trim()} onclick={reviewPasted}>Review pasted record</button>
    {#if recordError}<p class="error">{recordError}</p>{/if}
  </section>
</div>

<style>
  .lobby {
    padding: 16px 16px 32px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 480px;
    margin: 0 auto;
  }
  h1 {
    margin: 8px 0 0;
    font-size: 2rem;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 1rem;
  }
  .levels {
    display: grid;
    gap: 8px;
    margin-bottom: 12px;
  }
  .level {
    min-height: 52px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 12px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--felt-dark);
    color: #f4f1ea;
    --muted: #cfd8cf;
    font: inherit;
    text-align: left;
  }
  .level.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .lname {
    font-weight: 700;
  }
  .lhint {
    font-size: 0.78rem;
    color: var(--muted);
  }
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 44px;
    font-size: 0.9rem;
  }
  .field.col {
    flex-direction: column;
    align-items: stretch;
  }
  .field.check input {
    width: 24px;
    height: 24px;
    min-height: 0;
    accent-color: var(--accent);
  }
  .field small {
    color: var(--muted);
    font-size: 0.75rem;
  }
  .field input,
  .field select,
  .field textarea {
    min-height: 40px;
    border-radius: 8px;
    border: 1px solid var(--line);
    background: var(--card-bg);
    color: var(--card-fg);
    padding: 6px 10px;
    font: inherit;
    max-width: 60%;
  }
  .field.col textarea {
    max-width: none;
    font-family: ui-monospace, monospace;
    font-size: 0.8rem;
  }
  .error {
    color: var(--c-blunder);
    font-size: 0.85rem;
  }
</style>
