<script lang="ts">
  import { isRed, parseCard, SUIT_SYMBOL, type Card } from '../engine/types';

  interface Props {
    card?: Card | null;
    faceDown?: boolean;
    selectable?: boolean;
    dim?: boolean;
    small?: boolean;
    empty?: boolean;
    onselect?: (card: Card) => void;
  }

  let { card = null, faceDown = false, selectable = false, dim = false, small = false, empty = false, onselect }: Props =
    $props();

  const parsed = $derived(card && !faceDown ? parseCard(card) : null);
  const red = $derived(card && !faceDown ? isRed(card) : false);
  const label = $derived(parsed ? `${parsed.rank}${SUIT_SYMBOL[parsed.suit]}` : faceDown ? 'face-down card' : 'empty');

  function click() {
    if (selectable && card && onselect) onselect(card);
  }
</script>

{#if empty || (!card && !faceDown)}
  <div class="card empty" class:small aria-label="empty slot"></div>
{:else if faceDown || !parsed}
  <div class="card back" class:small aria-label="face-down card"></div>
{:else}
  <button
    type="button"
    class="card face"
    class:red
    class:selectable
    class:dim
    class:small
    class:manille={parsed.rank === '10'}
    aria-label={label}
    disabled={!selectable}
    onclick={click}
  >
    <span class="corner">{parsed.rank}<span class="suit">{SUIT_SYMBOL[parsed.suit]}</span></span>
    <span class="pip">{SUIT_SYMBOL[parsed.suit]}</span>
    {#if parsed.rank === '10'}<span class="m">M</span>{/if}
  </button>
{/if}

<style>
  .card {
    --w: var(--card-w, 56px);
    width: var(--w);
    height: calc(var(--w) * 1.4);
    border-radius: 6px;
    box-sizing: border-box;
    position: relative;
    flex: 0 0 auto;
    font-family: inherit;
  }
  .card.small {
    --w: var(--card-w-small, 40px);
  }
  .face {
    background: var(--card-bg);
    color: var(--card-fg);
    border: 1px solid var(--card-border);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
    padding: 0;
    cursor: default;
    text-align: left;
    transition: transform 120ms ease, box-shadow 120ms ease;
  }
  .face.red {
    color: var(--card-red);
  }
  .face.selectable {
    cursor: pointer;
    box-shadow: 0 0 0 2px var(--accent), 0 2px 6px rgba(0, 0, 0, 0.4);
  }
  .face.selectable:active {
    transform: translateY(-4px);
  }
  .face.dim {
    opacity: 0.45;
  }
  .face:disabled {
    cursor: default;
  }
  .corner {
    position: absolute;
    top: 3px;
    left: 4px;
    font-size: calc(var(--w) * 0.3);
    font-weight: 700;
    line-height: 1;
  }
  .corner .suit {
    display: block;
    font-size: calc(var(--w) * 0.26);
  }
  .pip {
    position: absolute;
    right: 5px;
    bottom: 2px;
    font-size: calc(var(--w) * 0.5);
    line-height: 1;
  }
  .m {
    position: absolute;
    top: 3px;
    right: 5px;
    font-size: calc(var(--w) * 0.22);
    font-weight: 800;
    color: var(--accent-strong);
  }
  .back {
    background: repeating-linear-gradient(45deg, var(--card-back-a) 0 4px, var(--card-back-b) 4px 8px);
    border: 2px solid var(--card-bg);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
  }
  .empty {
    border: 1px dashed var(--muted);
    opacity: 0.5;
  }
</style>
