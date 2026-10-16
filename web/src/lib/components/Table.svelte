<script lang="ts">
  import Card from './Card.svelte';
  import {
    bidLabel,
    cardLabel,
    compareCards,
    otherPlayer,
    sameBid,
    type Card as CardT,
    type ParReport,
    type PlayerView,
    type TableSlot,
    type Trick,
  } from '../engine/types';

  interface Props {
    view: PlayerView;
    scores?: [number, number];
    target?: number;
    aiThinking?: boolean;
    readonly?: boolean;
    /** A just-completed trick to keep on the table until the next lead. */
    completedTrick?: Trick | null;
    /** Hindsight par of the deal; shown as one line when provided. */
    par?: ParReport | null;
    parPending?: boolean;
    onplay?: (card: CardT) => void;
  }

  let {
    view,
    scores = [0, 0],
    target = 101,
    aiThinking = false,
    readonly = false,
    completedTrick = null,
    par = null,
    parPending = false,
    onplay,
  }: Props = $props();

  const parLine = $derived.by(() => {
    if (!par) return null;
    const iAmDealer = par.dealer === view.me;
    const sign = (n: number) => (n > 0 ? `+${n}` : `${n}`);
    if (view.bid) {
      const row = par.actual ?? par.bids.find((b) => sameBid(b.bid, view.bid)) ?? null;
      if (!row) return null;
      const my = iAmDealer ? row.dealer_score : -row.dealer_score;
      return `Par: ${sign(my)} for you on ${bidLabel(row.bid)} · perfect play gives you ${row.points[view.me]} pts`;
    }
    const best = par.bids.find((b) => sameBid(b.bid, par.best_bid)) ?? null;
    const my = iAmDealer ? par.par_score : -par.par_score;
    const who = iAmDealer ? 'your' : "dealer's";
    return `Par: ${who} best bid ${bidLabel(par.best_bid)} for ${sign(my)} · perfect play gives you ${best ? best.points[view.me] : '?'} pts`;
  });

  const me = $derived(view.me);
  const opp = $derived(otherPlayer(view.me));
  const legal = $derived(new Set(view.legal_moves.filter((m) => m.type === 'play').map((m) => (m as { card: CardT }).card)));
  const myTurn = $derived(!readonly && view.to_move === view.me && view.phase === 'play');
  const hand = $derived([...view.hand].sort(compareCards));
  const myRow = $derived(view.table[me]);
  const oppRow = $derived(view.table[opp]);

  let showLast = $state(false);

  function isLegal(card: CardT | null): boolean {
    return myTurn && card !== null && legal.has(card);
  }

  function play(card: CardT) {
    if (isLegal(card) && onplay) onplay(card);
  }

  function slotKey(slot: TableSlot, i: number): string {
    return `${i}:${slot.top ?? '-'}:${slot.covered}`;
  }

  const trickCards = $derived.by(() => {
    const t = view.current_trick;
    if (t) return { lead: t.cards[0] ?? null, follow: t.cards[1] ?? null, leader: t.leader, winner: null as number | null };
    const done = completedTrick;
    if (done && view.phase !== 'done') return { lead: done.cards[0], follow: done.cards[1], leader: done.leader, winner: done.winner };
    return { lead: null as CardT | null, follow: null as CardT | null, leader: null as number | null, winner: null as number | null };
  });
</script>

<div class="table">
  <div class="strip">
    <div class="who">
      <span class="name">Opponent</span>
      <span class="pts">{view.points[opp]} pts</span>
      <span class="match">match {scores[opp]} / {target}</span>
    </div>
    <div class="bid">
      <span class="bidlabel">{bidLabel(view.bid)}</span>
      {#if view.contra}<span class="tag">contra</span>{/if}
      {#if view.multiplier > 1}<span class="tag mult">×{view.multiplier}</span>{/if}
    </div>
    <div class="who right">
      <span class="name">You</span>
      <span class="pts">{view.points[me]} pts</span>
      <span class="match">match {scores[me]} / {target}</span>
    </div>
  </div>

  {#if parLine || parPending}
    <div class="parline" title="Hindsight: computed with every card visible and both sides playing perfectly. Not information you would have at the table.">
      {#if parLine}{parLine}{:else}computing par…{/if}
    </div>
  {/if}

  <section class="row opp" aria-label="opponent table">
    <div class="handcount">
      {#each { length: Math.min(view.opp_hand_count, 8) } as _, i (i)}
        <div class="mini back"></div>
      {/each}
      <span class="count">{view.opp_hand_count} in hand</span>
    </div>
    <div class="slots">
      {#each oppRow as slot, i (slotKey(slot, i))}
        <div class="slot" class:covered={slot.covered}>
          {#if slot.covered}<div class="under"></div>{/if}
          <Card card={slot.top} empty={slot.top === null} small />
        </div>
      {/each}
    </div>
  </section>

  <section class="centre" aria-label="current trick">
    <div class="trick">
      {#if trickCards.lead}
        <div class="played" class:mine={trickCards.leader === me}>
          <Card card={trickCards.lead} small />
          <span class="by">{trickCards.leader === me ? 'you' : 'opp'}</span>
        </div>
      {/if}
      {#if trickCards.follow}
        <div class="played" class:mine={trickCards.leader !== me}>
          <Card card={trickCards.follow} small />
          <span class="by">{trickCards.leader !== me ? 'you' : 'opp'}</span>
        </div>
      {/if}
      {#if trickCards.winner !== null}
        <span class="hint won" class:mine={trickCards.winner === me}>{trickCards.winner === me ? 'You win the trick' : 'Opponent wins the trick'}</span>
      {/if}
      {#if !trickCards.lead}
        <span class="hint">
          {#if view.phase === 'done'}Deal over
          {:else if aiThinking}Opponent is thinking…
          {:else if myTurn}Your lead
          {:else if view.phase === 'play'}Waiting for opponent
          {:else}{view.phase}{/if}
        </span>
      {:else if aiThinking}
        <span class="hint">Opponent is thinking…</span>
      {:else if myTurn}
        <span class="hint">Your turn</span>
      {/if}
    </div>
    <div class="centre-side">
      <span class="trickno">trick {Math.min(view.tricks_played + 1, 16)}/16</span>
      <button type="button" class="peek" disabled={!view.last_trick} onclick={() => (showLast = !showLast)}>
        {showLast ? 'Hide' : 'Last trick'}
      </button>
    </div>
    {#if showLast && view.last_trick}
      <div class="last" role="dialog" aria-label="last trick">
        <span class="lbl">Last trick · led by {view.last_trick.leader === me ? 'you' : 'opp'}</span>
        <div class="lastcards">
          <Card card={view.last_trick.cards[0]} small />
          <Card card={view.last_trick.cards[1]} small />
        </div>
        <span class="lbl">won by {view.last_trick.winner === me ? 'you' : 'opp'} · {view.last_trick.points} pts
          ({cardLabel(view.last_trick.cards[0])}, {cardLabel(view.last_trick.cards[1])})</span>
      </div>
    {/if}
  </section>

  <section class="row mine" aria-label="your table">
    <div class="slots">
      {#each myRow as slot, i (slotKey(slot, i))}
        <div class="slot" class:covered={slot.covered}>
          {#if slot.covered}<div class="under"></div>{/if}
          <Card
            card={slot.top}
            empty={slot.top === null}
            small
            selectable={isLegal(slot.top)}
            dim={myTurn && !isLegal(slot.top)}
            onselect={play}
          />
        </div>
      {/each}
    </div>
  </section>

  <section class="hand" aria-label="your hand">
    {#each hand as card (card)}
      <Card {card} selectable={isLegal(card)} dim={myTurn && !isLegal(card)} onselect={play} />
    {/each}
    {#if hand.length === 0}<span class="hint">no hand cards left</span>{/if}
  </section>
</div>

<style>
  .table {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 12px 12px;
    min-height: 100%;
    box-sizing: border-box;
  }
  .strip {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
  }
  .who {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  .who.right {
    text-align: right;
  }
  .name {
    font-weight: 700;
  }
  .pts {
    font-variant-numeric: tabular-nums;
  }
  .match {
    color: var(--muted);
    font-size: 0.75rem;
  }
  .bid {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--panel);
    border-radius: 999px;
    padding: 6px 12px;
  }
  .bidlabel {
    font-weight: 700;
  }
  .tag {
    background: var(--accent);
    color: var(--accent-fg);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 0.75rem;
    font-weight: 700;
  }
  .tag.mult {
    background: var(--accent-strong);
  }
  .parline {
    text-align: center;
    font-size: 0.72rem;
    color: var(--muted);
    font-style: italic;
  }
  .row {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .slots {
    display: flex;
    gap: 10px;
    justify-content: center;
  }
  .slot {
    position: relative;
    padding-top: 6px;
  }
  .slot .under {
    position: absolute;
    top: 0;
    left: 4px;
    right: -4px;
    height: calc(var(--card-w-small, 40px) * 1.4);
    border-radius: 6px;
    background: repeating-linear-gradient(45deg, var(--card-back-a) 0 4px, var(--card-back-b) 4px 8px);
    border: 2px solid var(--card-bg);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
  }
  .slot :global(.card) {
    position: relative;
  }
  .handcount {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .mini.back {
    width: 14px;
    height: 20px;
    border-radius: 3px;
    background: repeating-linear-gradient(45deg, var(--card-back-a) 0 3px, var(--card-back-b) 3px 6px);
    border: 1px solid var(--card-bg);
  }
  .count {
    margin-left: 8px;
    color: var(--muted);
    font-size: 0.75rem;
  }
  .centre {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: var(--felt-dark);
    border-radius: 14px;
    padding: 10px;
    min-height: 120px;
    position: relative;
  }
  .won {
    font-weight: 600;
  }
  .won.mine {
    color: var(--accent, #4caf50);
  }
  .trick {
    display: flex;
    gap: 16px;
    align-items: center;
    min-height: 64px;
  }
  .played {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    font-size: 0.7rem;
    color: var(--muted);
  }
  .played.mine {
    color: var(--fg);
  }
  .hint {
    color: var(--muted);
    font-size: 0.85rem;
  }
  .centre-side {
    display: flex;
    gap: 12px;
    align-items: center;
    font-size: 0.75rem;
    color: var(--muted);
  }
  .peek {
    min-height: 36px;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px solid var(--muted);
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 0.8rem;
  }
  .peek:disabled {
    opacity: 0.4;
  }
  .last {
    position: absolute;
    inset: 6px;
    background: var(--panel);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 0.8rem;
    z-index: 2;
  }
  .lastcards {
    display: flex;
    gap: 10px;
  }
  .lbl {
    color: var(--muted);
    text-align: center;
  }
  .hand {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    justify-content: center;
    padding: 6px 0;
    min-height: 90px;
    align-items: center;
  }
</style>
