// TypeScript mirror of docs/api.md. Keep in sync with the Rust serde types.

export type Card = string; // "7C", "JD", "AH", "10S"
export type Suit = 'C' | 'D' | 'H' | 'S';
export type Rank = '7' | '8' | '9' | 'J' | 'Q' | 'K' | 'A' | '10';
export type Player = 0 | 1;
export type Phase = 'bidding' | 'contra' | 'play' | 'done';
export type Level = 'easy' | 'medium' | 'hard';
export type Label = 'best' | 'good' | 'inaccuracy' | 'mistake' | 'blunder';
export type Unit = 'points' | 'score';

export type Bid = { type: 'trump'; suit: Suit } | { type: 'no_trump' };

export type Move =
  | { type: 'bid'; bid: Bid }
  | { type: 'contra'; contra: boolean }
  | { type: 'play'; card: Card };

export interface TableSlot {
  top: Card | null;
  covered: boolean;
}

export interface Trick {
  leader: Player;
  cards: Card[];
  winner: Player;
  points: number;
}

export interface CurrentTrick {
  leader: Player;
  cards: Card[];
}

export interface PlayerView {
  me: Player;
  dealer: Player;
  phase: Phase;
  to_move: Player | null;
  bid: Bid | null;
  contra: boolean;
  multiplier: number;
  hand: Card[];
  opp_hand_count: number;
  table: [TableSlot[], TableSlot[]];
  current_trick: CurrentTrick | null;
  last_trick: Trick | null;
  tricks_played: number;
  points: [number, number];
  played: Card[];
  unknown: Card[];
  legal_moves: Move[];
}

export interface DealResult {
  dealer: Player;
  bid: Bid;
  contra: boolean;
  multiplier: number;
  points: [number, number];
  winner: Player | null;
  base: number;
  score: number;
}

export interface MatchState {
  target: number;
  scores: [number, number];
  dealer: Player;
  deal_index: number;
  phase: Phase;
  finished: boolean;
  winner: Player | null;
  deals: DealResult[];
}

export interface LayoutSlot {
  top: Card;
  under: Card;
}

export interface Layout {
  hands: [Card[], Card[]];
  table: [LayoutSlot[], LayoutSlot[]];
}

export interface GameRecord {
  version: number;
  dealer: Player;
  layout: Layout;
  moves: Move[];
}

export interface EvalOption {
  move: Move;
  value: number;
  /** For a lead: sampled layouts in which the opponent wins the trick (reply is forced by the rules). */
  trick_lost?: number;
}

/** One bid solved with perfect information (both seats see every card, non-dealer leads). */
export interface ParBid {
  bid: Bid;
  /** Exact card points per seat under perfect play. */
  points: [number, number];
  /** Whether the non-dealer's optimal choice is to contra. */
  contra: boolean;
  /** Signed deal score for the dealer, multiplier included. */
  dealer_score: number;
}

/** Hindsight par of a deal. Uses hidden information: show only on request or after the deal. */
export interface ParReport {
  dealer: Player;
  bids: ParBid[];
  best_bid: Bid;
  par_score: number;
  /** The bid/contra actually made, solved perfectly; null before the bidding is done. */
  actual: ParBid | null;
  nodes: number;
}

export interface EvalResponse {
  player: Player;
  unit: Unit;
  options: EvalOption[];
  samples: number;
  nodes: number;
}

export interface AiMoveResponse {
  move: Move;
  eval: EvalResponse | null;
}

export interface Decision {
  index: number;
  trick: number;
  phase: Phase;
  chosen: Move;
  best: Move;
  options: EvalOption[];
  value_chosen: number;
  value_best: number;
  loss: number;
  label: Label;
  unit: Unit;
  note: string | null;
  /** Why the best move is good; derived only from the player's own view. */
  reasons_best?: string[];
  /** What the chosen move did; empty when it was the best move. */
  reasons_chosen?: string[];
  /** Sampled layouts of the unseen cards the values are averaged over. */
  samples?: number;
}

export interface TimelinePoint {
  index: number;
  value: number;
}

export interface AnalysisSummary {
  best: number;
  good: number;
  inaccuracy: number;
  mistake: number;
  blunder: number;
  avg_loss: number;
}

export interface AnalysisReport {
  player: Player;
  decisions: Decision[];
  timeline: TimelinePoint[];
  summary: AnalysisSummary;
}

export interface Settings {
  level: Level;
  target: number;
  human: Player;
  max_nodes: number;
  /** Dealer of deal 0; null = engine picks. */
  first_dealer?: Player | null;
  /** GUI only: reveal the hindsight par line while playing. Ignored by the engine. */
  show_par?: boolean;
}

/** Output of `snapshot_json()`, accepted by `Session.restore`. Treated as opaque by the UI. */
export interface SessionSnapshot {
  settings: Settings;
  match: {
    seed: [number, number];
    target: number;
    first_dealer: Player;
    scores: [number, number];
    deals: DealResult[];
    record: GameRecord;
  };
}

export type Event =
  | { type: 'bid'; player: Player; bid: Bid }
  | { type: 'contra'; player: Player; contra: boolean }
  | { type: 'played'; player: Player; card: Card; from_table: boolean }
  | { type: 'flipped'; player: Player; slot: number; card: Card }
  | { type: 'trick'; trick: Trick }
  | { type: 'deal_over'; result: DealResult }
  | { type: 'match_over'; winner: Player };

export interface ApplyResponse {
  events: Event[];
  view: PlayerView;
}

// ---- helpers ----

export const SUITS: Suit[] = ['C', 'D', 'H', 'S'];
export const RANKS: Rank[] = ['7', '8', '9', 'J', 'Q', 'K', 'A', '10'];

const POINTS: Record<Rank, number> = { '7': 0, '8': 0, '9': 0, J: 1, Q: 2, K: 3, A: 4, '10': 5 };

export const SUIT_SYMBOL: Record<Suit, string> = { C: '♣', D: '♦', H: '♥', S: '♠' };
export const SUIT_NAME: Record<Suit, string> = { C: 'clubs', D: 'diamonds', H: 'hearts', S: 'spades' };

export function parseCard(card: Card): { rank: Rank; suit: Suit } {
  const suit = card.slice(-1) as Suit;
  const rank = card.slice(0, -1) as Rank;
  if (!SUITS.includes(suit) || !RANKS.includes(rank)) {
    throw new Error(`invalid card: ${card}`);
  }
  return { rank, suit };
}

export function cardPoints(card: Card): number {
  return POINTS[parseCard(card).rank];
}

/** 0 for the 7 up to 7 for the 10 (Manille). */
export function rankOrder(card: Card): number {
  return RANKS.indexOf(parseCard(card).rank);
}

export function isRed(card: Card): boolean {
  const { suit } = parseCard(card);
  return suit === 'D' || suit === 'H';
}

/** Human readable, e.g. "10♥". */
export function cardLabel(card: Card): string {
  const { rank, suit } = parseCard(card);
  return `${rank}${SUIT_SYMBOL[suit]}`;
}

/** Sort key: suit first, then rank ascending. */
export function compareCards(a: Card, b: Card): number {
  const pa = parseCard(a);
  const pb = parseCard(b);
  if (pa.suit !== pb.suit) return SUITS.indexOf(pa.suit) - SUITS.indexOf(pb.suit);
  return rankOrder(a) - rankOrder(b);
}

export function moveKey(move: Move): string {
  switch (move.type) {
    case 'bid':
      return move.bid.type === 'trump' ? `bid:${move.bid.suit}` : 'bid:NT';
    case 'contra':
      return `contra:${move.contra}`;
    case 'play':
      return `play:${move.card}`;
  }
}

export function sameMove(a: Move, b: Move): boolean {
  return moveKey(a) === moveKey(b);
}

export function moveLabel(move: Move): string {
  switch (move.type) {
    case 'bid':
      return move.bid.type === 'trump' ? `Trump ${SUIT_SYMBOL[move.bid.suit]}` : 'No trump';
    case 'contra':
      return move.contra ? 'Contra' : 'No contra';
    case 'play':
      return cardLabel(move.card);
  }
}

export function sameBid(a: Bid | null, b: Bid | null): boolean {
  if (!a || !b) return false;
  return a.type === 'no_trump' ? b.type === 'no_trump' : b.type === 'trump' && a.suit === b.suit;
}

export function bidLabel(bid: Bid | null): string {
  if (!bid) return '—';
  return bid.type === 'trump' ? `${SUIT_SYMBOL[bid.suit]} trump` : 'No trump';
}

export function otherPlayer(p: Player): Player {
  return p === 0 ? 1 : 0;
}

export const DEFAULT_SETTINGS: Settings = { level: 'medium', target: 101, human: 0, max_nodes: 1_500_000, first_dealer: null, show_par: false };
