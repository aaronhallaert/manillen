<script lang="ts">
  import { SUIT_NAME, SUIT_SYMBOL, SUITS, type Bid, type Move } from '../engine/types';

  interface Props {
    legal: Move[];
    onbid: (bid: Bid) => void;
  }

  let { legal, onbid }: Props = $props();

  const allowed = $derived(
    new Set(legal.filter((m) => m.type === 'bid').map((m) => ((m as { bid: Bid }).bid.type === 'trump' ? (m as { bid: { suit: string } }).bid.suit : 'NT'))),
  );
</script>

<div class="banner" role="region" aria-label="choose trump">
  <div class="dialog">
    <h2>You deal — choose trump</h2>
    <p class="sub">No passing. No trump doubles the score.</p>
    <div class="grid">
      {#each SUITS as suit (suit)}
        <button
          type="button"
          class="opt"
          class:red={suit === 'D' || suit === 'H'}
          disabled={!allowed.has(suit)}
          onclick={() => onbid({ type: 'trump', suit })}
        >
          <span class="sym">{SUIT_SYMBOL[suit]}</span>
          <span>{SUIT_NAME[suit]}</span>
        </button>
      {/each}
      <button type="button" class="opt wide" disabled={!allowed.has('NT')} onclick={() => onbid({ type: 'no_trump' })}>
        <span class="sym">×2</span>
        <span>No trump</span>
      </button>
    </div>
  </div>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 8px;
  }
  .opt {
    min-height: 48px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border-radius: 12px;
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    color: var(--card-fg);
    font: inherit;
    font-weight: 600;
  }
  .opt.red {
    color: var(--card-red);
  }
  .opt.wide {
    grid-column: auto;
  }
  @media (max-width: 420px) {
    .grid {
      grid-template-columns: repeat(4, 1fr);
    }
    .opt.wide {
      grid-column: 1 / -1;
    }
  }
  .opt:disabled {
    opacity: 0.4;
  }
  .sym {
    font-size: 1.6rem;
    line-height: 1;
  }
</style>
