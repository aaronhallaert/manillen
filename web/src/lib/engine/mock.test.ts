import { describe, expect, it } from 'vitest';
import { MockEngine } from './mock';
import { DEFAULT_SETTINGS } from './types';

describe('MockEngine', () => {
  it('walks bidding → contra → play → done', async () => {
    const e = new MockEngine({ delay: 0 });
    await e.newMatch({ ...DEFAULT_SETTINGS, human: 0 });
    let v = await e.view(0);
    expect(v.phase).toBe('bidding');
    expect(v.legal_moves).toHaveLength(5);

    let r = await e.apply({ type: 'bid', bid: { type: 'no_trump' } });
    expect(r.view.phase).toBe('contra');
    expect(r.events[0]).toMatchObject({ type: 'bid' });

    r = await e.apply({ type: 'contra', contra: true });
    expect(r.view.phase).toBe('play');
    expect(r.view.multiplier).toBe(4);
    expect(r.view.legal_moves.length).toBeGreaterThan(0);

    await expect(e.apply({ type: 'play', card: 'ZZ' })).rejects.toThrow(/illegal/);

    let events = r.events;
    for (let i = 0; i < 4; i++) {
      v = await e.view(0);
      const mv = v.legal_moves[0];
      r = await e.apply(mv);
      events = r.events;
    }
    expect(r.view.phase).toBe('done');
    expect(events.some((ev) => ev.type === 'deal_over')).toBe(true);
    const m = await e.match();
    expect(m.deals).toHaveLength(1);

    const rec = await e.record();
    expect(rec.moves).toHaveLength(6);

    const next = await e.nextDeal();
    expect(next.deal_index).toBe(1);
    expect((await e.view(0)).phase).toBe('bidding');
  });

  it('mirrors seats when the human sits in seat 1', async () => {
    const e = new MockEngine({ delay: 0 });
    await e.newMatch({ ...DEFAULT_SETTINGS, human: 1 });
    const v = await e.view(1);
    expect(v.me).toBe(1);
    expect(v.to_move).toBe(1);
  });

  it('returns a legal ai move and an analysis', async () => {
    const e = new MockEngine({ delay: 0 });
    await e.newMatch(DEFAULT_SETTINGS);
    const { move } = await e.aiMove('hard', 1000);
    expect(move.type).toBe('bid');
    const par = await e.par(0);
    expect(par.bids).toHaveLength(5);
    expect(par.actual).toBeNull();
    const parRec = await e.parOfRecord({ ...(await e.record()), moves: [{ type: 'bid', bid: { type: 'trump', suit: 'H' } }] }, 0);
    expect(parRec.actual?.bid).toEqual({ type: 'trump', suit: 'H' });
    const a = await e.analyze(await e.record(), 0, 1000);
    expect(a.decisions.length).toBeGreaterThan(0);
    expect(a.summary.best).toBe(2);
  });
});
