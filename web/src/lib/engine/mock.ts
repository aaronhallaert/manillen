import type { Engine, Seed } from './client';
import type {
  AiMoveResponse,
  AnalysisReport,
  ApplyResponse,
  DealResult,
  EvalResponse,
  Event,
  GameRecord,
  Level,
  MatchState,
  Move,
  ParReport,
  Player,
  PlayerView,
  Settings,
} from './types';
import { DEFAULT_SETTINGS } from './types';
import viewBidding from './fixtures/view_bidding.json';
import viewContra from './fixtures/view_contra.json';
import viewPlay from './fixtures/view_play.json';
import analysisFixture from './fixtures/analysis.json';
import evalFixture from './fixtures/eval.json';
import matchFixture from './fixtures/match.json';
import recordFixture from './fixtures/record.json';
import dealResultFixture from './fixtures/deal_result.json';
import parFixture from './fixtures/par.json';

function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Fixture-driven engine: bidding → contra → play → done. Good enough to build the UI against. */
export class MockEngine implements Engine {
  readonly kind = 'mock' as const;
  /** Delay (ms) for "thinking"; tests set it to 0. */
  delay = 300;

  private settings: Settings = DEFAULT_SETTINGS;
  private current: PlayerView = clone(viewBidding as unknown as PlayerView);
  private matchState: MatchState = clone(matchFixture as MatchState);
  private moves: Move[] = [];
  private plays = 0;

  constructor(opts: { delay?: number } = {}) {
    if (opts.delay !== undefined) this.delay = opts.delay;
  }

  async newMatch(settings: Settings, _seed?: Seed): Promise<MatchState> {
    this.settings = settings;
    this.moves = [];
    this.plays = 0;
    this.matchState = { ...clone(matchFixture as MatchState), scores: [0, 0], deals: [], deal_index: 0, phase: 'bidding' };
    this.current = this.relabel(clone(viewBidding as unknown as PlayerView));
    return clone(this.matchState);
  }

  async match(): Promise<MatchState> {
    return clone({ ...this.matchState, phase: this.current.phase });
  }

  async view(_player: Player): Promise<PlayerView> {
    return clone(this.current);
  }

  async legalMoves(): Promise<Move[]> {
    return clone(this.current.legal_moves);
  }

  async apply(move: Move): Promise<ApplyResponse> {
    const me = this.settings.human;
    const events: Event[] = [];
    switch (move.type) {
      case 'bid': {
        if (this.current.phase !== 'bidding') throw new Error('not in bidding phase');
        events.push({ type: 'bid', player: me, bid: move.bid });
        this.current = this.relabel({ ...clone(viewContra as unknown as PlayerView), bid: move.bid });
        break;
      }
      case 'contra': {
        if (this.current.phase !== 'contra') throw new Error('not in contra phase');
        events.push({ type: 'contra', player: me, contra: move.contra });
        const v = this.relabel(clone(viewPlay as unknown as PlayerView));
        v.bid = this.current.bid;
        v.contra = move.contra;
        v.multiplier = (v.bid?.type === 'no_trump' ? 2 : 1) * (move.contra ? 2 : 1);
        this.current = v;
        break;
      }
      case 'play': {
        if (this.current.phase !== 'play') throw new Error('not in play phase');
        if (!this.current.legal_moves.some((m) => m.type === 'play' && m.card === move.card)) {
          throw new Error(`illegal move: ${move.card}`);
        }
        this.playCard(move.card, events);
        break;
      }
    }
    this.moves.push(move);
    return { events, view: clone(this.current) };
  }

  async nextDeal(): Promise<MatchState> {
    if (this.current.phase !== 'done') throw new Error('deal not finished');
    this.matchState.deal_index += 1;
    this.matchState.dealer = this.matchState.dealer === 0 ? 1 : 0;
    this.moves = [];
    this.plays = 0;
    this.current = this.relabel(clone(viewBidding as unknown as PlayerView));
    return this.match();
  }

  async aiMove(_level: Level, _maxNodes: number): Promise<AiMoveResponse> {
    await sleep(this.delay);
    const legal = this.current.legal_moves;
    if (legal.length === 0) throw new Error('no legal moves');
    const move = legal[Math.floor(Math.random() * legal.length)];
    return { move, eval: null };
  }

  async evaluate(_player: Player, _maxNodes: number): Promise<EvalResponse> {
    await sleep(this.delay);
    return clone(evalFixture as EvalResponse);
  }

  async record(): Promise<GameRecord> {
    const rec = clone(recordFixture as GameRecord);
    rec.moves = clone(this.moves);
    return rec;
  }

  async analyze(_record: GameRecord, player: Player, _maxNodes: number): Promise<AnalysisReport> {
    await sleep(this.delay);
    return { ...clone(analysisFixture as AnalysisReport), player };
  }

  async viewAt(_record: GameRecord, _index: number, player: Player): Promise<PlayerView> {
    const v = clone(viewPlay as unknown as PlayerView);
    v.me = player;
    v.legal_moves = [];
    return v;
  }

  async par(_maxNodes: number): Promise<ParReport> {
    await sleep(this.delay);
    const par = clone(parFixture as ParReport);
    par.dealer = this.current.dealer;
    const bid = this.current.bid;
    par.actual = bid ? (par.bids.find((b) => JSON.stringify(b.bid) === JSON.stringify(bid)) ?? null) : null;
    return par;
  }

  async parOfRecord(record: GameRecord, _maxNodes: number): Promise<ParReport> {
    await sleep(this.delay);
    const par = clone(parFixture as ParReport);
    par.dealer = record.dealer;
    const bidMove = record.moves.find((m) => m.type === 'bid');
    par.actual = bidMove && bidMove.type === 'bid' ? (par.bids.find((b) => JSON.stringify(b.bid) === JSON.stringify(bidMove.bid)) ?? null) : null;
    return par;
  }

  async snapshot(): Promise<string> {
    return JSON.stringify({
      settings: this.settings,
      current: this.current,
      matchState: this.matchState,
      moves: this.moves,
      plays: this.plays,
    });
  }

  async restore(snapshot: string): Promise<MatchState> {
    const s = JSON.parse(snapshot) as {
      settings: Settings;
      current: PlayerView;
      matchState: MatchState;
      moves: Move[];
      plays: number;
    };
    if (!s.current || !s.matchState) throw new Error('invalid snapshot');
    this.settings = s.settings;
    this.current = s.current;
    this.matchState = s.matchState;
    this.moves = s.moves ?? [];
    this.plays = s.plays ?? 0;
    return this.match();
  }

  async version(): Promise<string> {
    return 'mock';
  }

  dispose(): void {}

  // ---- internals ----

  /** Fixtures are written for me = 0; mirror them when the human sits in seat 1. */
  private relabel(v: PlayerView): PlayerView {
    const me = this.settings.human;
    if (me === 0) return v;
    const flip = (p: Player | null): Player | null => (p === null ? null : p === 0 ? 1 : 0);
    v.me = me;
    v.dealer = flip(v.dealer) as Player;
    v.to_move = flip(v.to_move);
    v.table = [v.table[1], v.table[0]];
    v.points = [v.points[1], v.points[0]];
    if (v.last_trick) {
      v.last_trick.leader = flip(v.last_trick.leader) as Player;
      v.last_trick.winner = flip(v.last_trick.winner) as Player;
    }
    if (v.current_trick) v.current_trick.leader = flip(v.current_trick.leader) as Player;
    return v;
  }

  private playCard(card: string, events: Event[]): void {
    const me = this.settings.human;
    const v = this.current;
    const handIdx = v.hand.indexOf(card);
    const row = v.table[me];
    let fromTable = false;
    if (handIdx >= 0) {
      v.hand.splice(handIdx, 1);
    } else {
      const slotIdx = row.findIndex((s) => s.top === card);
      fromTable = true;
      const slot = row[slotIdx];
      if (slot.covered) {
        // Reveal an arbitrary unknown card as the flipped one.
        const flipped = v.unknown.shift() ?? null;
        slot.top = flipped;
        slot.covered = false;
        if (flipped) events.push({ type: 'flipped', player: me, slot: slotIdx, card: flipped });
      } else {
        slot.top = null;
      }
    }
    events.push({ type: 'played', player: me, card, from_table: fromTable });
    v.played.push(card);
    this.plays += 1;

    // Opponent answers with an unknown card; the human wins every other trick.
    const opp = v.unknown.shift() ?? '7C';
    v.played.push(opp);
    v.opp_hand_count = Math.max(0, v.opp_hand_count - 1);
    const winner: Player = this.plays % 2 === 1 ? me : me === 0 ? 1 : 0;
    const trick = { leader: me, cards: [card, opp], winner, points: 2 };
    v.points[winner] += trick.points;
    v.last_trick = trick;
    v.current_trick = null;
    v.tricks_played += 1;
    events.push({ type: 'trick', trick });

    const available = [...v.hand, ...row.filter((s) => s.top).map((s) => s.top as string)];
    if (v.tricks_played >= 16 || available.length === 0 || this.plays >= 4) {
      v.phase = 'done';
      v.to_move = null;
      v.legal_moves = [];
      const result: DealResult = {
        ...clone(dealResultFixture as DealResult),
        dealer: v.dealer,
        bid: v.bid ?? { type: 'no_trump' },
        contra: v.contra,
        multiplier: v.multiplier,
      };
      this.matchState.deals.push(result);
      if (result.winner !== null) this.matchState.scores[result.winner] += result.score;
      this.matchState.phase = 'done';
      events.push({ type: 'deal_over', result });
      if (this.matchState.scores[0] >= this.matchState.target || this.matchState.scores[1] >= this.matchState.target) {
        this.matchState.finished = true;
        this.matchState.winner = this.matchState.scores[0] >= this.matchState.target ? 0 : 1;
        events.push({ type: 'match_over', winner: this.matchState.winner });
      }
    } else {
      v.to_move = me;
      v.legal_moves = available.map((c) => ({ type: 'play', card: c }));
    }
  }
}
