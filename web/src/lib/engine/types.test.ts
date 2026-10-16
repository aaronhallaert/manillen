import { describe, expect, it } from 'vitest';
import { cardLabel, cardPoints, compareCards, moveKey, parseCard, rankOrder, sameMove } from './types';

describe('card helpers', () => {
  it('parses ranks and suits', () => {
    expect(parseCard('10H')).toEqual({ rank: '10', suit: 'H' });
    expect(parseCard('7C')).toEqual({ rank: '7', suit: 'C' });
    expect(() => parseCard('1X')).toThrow();
  });

  it('scores cards', () => {
    expect(cardPoints('10S')).toBe(5);
    expect(cardPoints('AS')).toBe(4);
    expect(cardPoints('KS')).toBe(3);
    expect(cardPoints('QS')).toBe(2);
    expect(cardPoints('JS')).toBe(1);
    expect(cardPoints('9S')).toBe(0);
  });

  it('ranks 10 above the ace', () => {
    expect(rankOrder('10D')).toBeGreaterThan(rankOrder('AD'));
    expect(rankOrder('AD')).toBeGreaterThan(rankOrder('KD'));
    expect(rankOrder('9D')).toBeGreaterThan(rankOrder('8D'));
    expect(['10C', '7C', 'AC'].sort(compareCards)).toEqual(['7C', 'AC', '10C']);
  });

  it('labels', () => {
    expect(cardLabel('10H')).toBe('10♥');
    expect(moveKey({ type: 'bid', bid: { type: 'no_trump' } })).toBe('bid:NT');
    expect(sameMove({ type: 'play', card: 'AS' }, { type: 'play', card: 'AS' })).toBe(true);
  });
});
