import { beforeEach, describe, expect, it } from 'vitest';
import { GameStore } from './game.svelte';
import { MockEngine } from '../engine/mock';

describe('GameStore', () => {
  let store: GameStore;

  beforeEach(async () => {
    localStorage.clear();
    store = new GameStore();
    await store.init(new MockEngine({ delay: 0 }));
  });

  it('starts a match and lands on the table', async () => {
    await store.newMatch();
    expect(store.screen).toBe('table');
    expect(store.view?.phase).toBe('bidding');
    expect(store.isMyTurn).toBe(true);
    expect(store.legalMoves).toHaveLength(5);
  });

  it('plays through a deal and records the result', async () => {
    await store.newMatch();
    await store.play({ type: 'bid', bid: { type: 'trump', suit: 'H' } });
    expect(store.view?.phase).toBe('contra');
    await store.play({ type: 'contra', contra: false });
    expect(store.view?.phase).toBe('play');
    while (store.view && store.view.phase === 'play') {
      await store.play(store.legalMoves[0]);
    }
    expect(store.view?.phase).toBe('done');
    expect(store.lastDealResult).not.toBeNull();
    expect(store.matchState?.deals).toHaveLength(1);
    expect(store.record?.moves.length).toBe(6);
    expect(localStorage.getItem('manillen.lastRecord')).not.toBeNull();

    await store.analyzeCurrentDeal();
    expect(store.screen).toBe('review');
    expect(store.pendingAnalysis).toBe(false);
    expect(store.analysis?.decisions.length).toBeGreaterThan(0);

    await store.nextDeal();
    expect(store.screen).toBe('table');
    expect(store.view?.phase).toBe('bidding');
  });

  it('ignores moves when it is not the human turn', async () => {
    await store.newMatch();
    store.view!.to_move = 1;
    await store.play({ type: 'bid', bid: { type: 'no_trump' } });
    expect(store.view?.phase).toBe('bidding');
  });

  it('persists settings', () => {
    store.updateSettings({ level: 'hard', target: 61 });
    const again = new GameStore();
    expect(again.settings.level).toBe('hard');
    expect(again.settings.target).toBe(61);
  });

  it('saves a snapshot and restores it in a fresh store', async () => {
    await store.newMatch();
    await store.play({ type: 'bid', bid: { type: 'trump', suit: 'S' } });
    const snap = localStorage.getItem('manillen.snapshot');
    expect(snap).not.toBeNull();

    const fresh = new GameStore();
    expect(fresh.canContinue).toBe(true);
    await fresh.init(new MockEngine({ delay: 0 }));
    await fresh.continueMatch();
    expect(fresh.screen).toBe('table');
    expect(fresh.view?.phase).toBe('contra');
    expect(fresh.view?.bid).toEqual({ type: 'trump', suit: 'S' });
  });

  it('fetches par when show_par is on, and at deal end otherwise', async () => {
    store.updateSettings({ show_par: true });
    await store.newMatch();
    await new Promise((r) => setTimeout(r, 0));
    expect(store.par).not.toBeNull();
    expect(store.par?.bids).toHaveLength(5);

    store.updateSettings({ show_par: false });
    await store.newMatch();
    expect(store.par).toBeNull();
    await store.play({ type: 'bid', bid: { type: 'no_trump' } });
    await store.play({ type: 'contra', contra: false });
    while (store.view && store.view.phase === 'play') await store.play(store.legalMoves[0]);
    await new Promise((r) => setTimeout(r, 0));
    expect(store.par?.actual?.bid).toEqual({ type: 'no_trump' });

    await store.analyzeCurrentDeal();
    await new Promise((r) => setTimeout(r, 0));
    expect(store.reviewPar?.best_bid).toEqual({ type: 'no_trump' });
  });

  it('surfaces engine errors', async () => {
    await store.newMatch();
    await store.play({ type: 'contra', contra: true });
    expect(store.error).toMatch(/phase/);
  });
});
