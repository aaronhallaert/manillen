import type {
  AiMoveResponse,
  AnalysisReport,
  ApplyResponse,
  EvalResponse,
  GameRecord,
  Level,
  MatchState,
  Move,
  ParReport,
  Player,
  PlayerView,
  Settings,
} from './types';

/** Seed as two u32 halves, matching `Session::new(seed_hi, seed_lo, ...)`. */
export interface Seed {
  hi: number;
  lo: number;
}

/** Promise-based mirror of the wasm `Session` (docs/api.md). */
export interface Engine {
  readonly kind: 'wasm' | 'mock';
  newMatch(settings: Settings, seed?: Seed): Promise<MatchState>;
  match(): Promise<MatchState>;
  view(player: Player): Promise<PlayerView>;
  legalMoves(): Promise<Move[]>;
  apply(move: Move): Promise<ApplyResponse>;
  nextDeal(): Promise<MatchState>;
  aiMove(level: Level, maxNodes: number): Promise<AiMoveResponse>;
  evaluate(player: Player, maxNodes: number): Promise<EvalResponse>;
  record(): Promise<GameRecord>;
  analyze(record: GameRecord, player: Player, maxNodes: number): Promise<AnalysisReport>;
  viewAt(record: GameRecord, index: number, player: Player): Promise<PlayerView>;
  /** Opaque snapshot string (SessionSnapshot JSON) for resuming after a reload. */
  /** Perfect-information par of the current deal (hindsight). maxNodes 0 = engine default. */
  par(maxNodes: number): Promise<ParReport>;
  /** Par of any record's layout. */
  parOfRecord(record: GameRecord, maxNodes: number): Promise<ParReport>;
  snapshot(): Promise<string>;
  /** Replaces the live session with the snapshot's one; returns the restored MatchState. */
  restore(snapshot: string): Promise<MatchState>;
  version(): Promise<string>;
  dispose(): void;
}

export function randomSeed(): Seed {
  const buf = new Uint32Array(2);
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) {
    crypto.getRandomValues(buf);
  } else {
    buf[0] = (Math.random() * 0xffffffff) >>> 0;
    buf[1] = (Math.random() * 0xffffffff) >>> 0;
  }
  return { hi: buf[0], lo: buf[1] };
}

/** Parse "1234" or "hi:lo" into a seed. */
export function parseSeed(text: string): Seed | null {
  const t = text.trim();
  if (!t) return null;
  const parts = t.split(':');
  if (parts.length === 2) {
    const hi = Number(parts[0]) >>> 0;
    const lo = Number(parts[1]) >>> 0;
    return { hi, lo };
  }
  const n = Number(t);
  if (!Number.isFinite(n)) return null;
  return { hi: Math.floor(n / 0x100000000) >>> 0, lo: n >>> 0 };
}

export function seedToString(seed: Seed): string {
  return `${seed.hi}:${seed.lo}`;
}
