<script lang="ts">
  import { onMount } from 'svelte';
  import { game } from './lib/state/game.svelte';
  import Lobby from './lib/components/Lobby.svelte';
  import Table from './lib/components/Table.svelte';
  import BidDialog from './lib/components/BidDialog.svelte';
  import ContraDialog from './lib/components/ContraDialog.svelte';
  import DealResult from './lib/components/DealResult.svelte';
  import Review from './lib/components/Review.svelte';
  import type { Bid, Card } from './lib/engine/types';

  onMount(() => {
    void game.init();
  });

  const showBid = $derived(game.screen === 'table' && game.view?.phase === 'bidding' && game.isMyTurn);
  const showContra = $derived(game.screen === 'table' && game.view?.phase === 'contra' && game.isMyTurn);
  const showResult = $derived(game.screen === 'table' && game.view?.phase === 'done' && game.lastDealResult !== null);

  function playCard(card: Card) {
    void game.play({ type: 'play', card });
  }
  function bid(b: Bid) {
    void game.play({ type: 'bid', bid: b });
  }
  function contra(c: boolean) {
    void game.play({ type: 'contra', contra: c });
  }
</script>

<main class:felt={game.screen === 'table'}>
  {#if !game.engine}
    <p class="loading">Loading engine…</p>
  {:else if game.screen === 'lobby'}
    <Lobby
      settings={game.settings}
      engineKind={game.engineKind}
      canContinue={game.canContinue}
      lastRecord={game.record}
      onsettings={(p) => game.updateSettings(p)}
      onstart={(seed) => void game.newMatch(seed)}
      oncontinue={() => void game.continueMatch()}
      onreview={(rec, player) => void game.openReview(rec, player)}
    />
  {:else if game.screen === 'table' && game.view}
    <div class="topbar">
      <button type="button" class="btn small" onclick={() => game.goTo('lobby')}>‹ Lobby</button>
      <span class="deal">deal {(game.matchState?.deal_index ?? 0) + 1} · {game.settings.level}</span>
    </div>
    <Table
      view={game.view}
      scores={game.matchState?.scores ?? [0, 0]}
      target={game.matchState?.target ?? game.settings.target}
      aiThinking={game.aiThinking}
      completedTrick={game.shownTrick}
      onplay={playCard}
    />
    {#if showBid}
      <BidDialog legal={game.legalMoves} onbid={bid} />
    {:else if showContra}
      <ContraDialog bid={game.view.bid} multiplier={game.view.multiplier} oncontra={contra} />
    {:else if showResult && game.lastDealResult}
      <DealResult
        result={game.lastDealResult}
        me={game.settings.human}
        match={game.matchState}
        par={game.par}
        parPending={game.parPending}
        onreview={() => void game.analyzeCurrentDeal()}
        onnext={() => void game.nextDeal()}
        onlobby={() => game.goTo('lobby')}
      />
    {/if}
  {:else if game.screen === 'review' && game.reviewRecord}
    <Review
      record={game.reviewRecord}
      analysis={game.analysis}
      pending={game.pendingAnalysis}
      par={game.reviewPar}
      parPending={game.reviewParPending}
      viewAt={(i) => game.viewAt(i)}
      onback={() => game.goTo(game.hasMatch ? 'table' : 'lobby')}
    />
  {:else}
    <Lobby
      settings={game.settings}
      engineKind={game.engineKind}
      canContinue={game.canContinue}
      lastRecord={game.record}
      onsettings={(p) => game.updateSettings(p)}
      onstart={(seed) => void game.newMatch(seed)}
      oncontinue={() => void game.continueMatch()}
      onreview={(rec, player) => void game.openReview(rec, player)}
    />
  {/if}

  {#if game.error}
    <div class="toast" role="alert">
      <span>{game.error}</span>
      <button type="button" class="btn small" onclick={() => game.clearError()}>ok</button>
    </div>
  {/if}
</main>

<style>
  main {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
  }
  main.felt {
    background: var(--felt);
  }
  .loading {
    padding: 24px;
    text-align: center;
    color: var(--muted);
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 0;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .toast {
    position: fixed;
    left: 12px;
    right: 12px;
    bottom: 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: var(--c-blunder);
    color: #fff;
    border-radius: 10px;
    padding: 10px 14px;
    font-size: 0.9rem;
    z-index: 20;
  }
</style>
