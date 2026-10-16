import type { Engine, Seed } from './client';
import { randomSeed } from './client';
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
import type { WorkerRequest, WorkerResponse } from './worker';

/** One Web Worker running worker.ts, with promise-based request/response matching. */
class WorkerChannel {
  private worker: Worker;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();

  constructor() {
    this.worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    this.worker.onmessage = (ev: MessageEvent<WorkerResponse>) => {
      const p = this.pending.get(ev.data.id);
      if (!p) return;
      this.pending.delete(ev.data.id);
      if (ev.data.ok) p.resolve(ev.data.result);
      else p.reject(new Error(ev.data.error ?? 'engine error'));
    };
    this.worker.onerror = (ev) => {
      const err = new Error(ev.message || 'worker error');
      for (const p of this.pending.values()) p.reject(err);
      this.pending.clear();
    };
  }

  call<T>(method: string, ...args: unknown[]): Promise<T> {
    const id = this.nextId++;
    const req: WorkerRequest = { id, method, args };
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.worker.postMessage(req);
    });
  }

  terminate(): void {
    this.worker.terminate();
    const err = new Error('worker terminated');
    for (const p of this.pending.values()) p.reject(err);
    this.pending.clear();
  }
}

/**
 * Engine backed by the wasm Session running inside Web Workers.
 *
 * The game worker holds the live match and answers quickly (views, moves, AI replies).
 * Slow, record-based work (par, analysis) runs on a second, lazily created analysis worker
 * so a multi-second par never delays the engine's next move.
 */
export class WasmEngine implements Engine {
  readonly kind = 'wasm' as const;
  private game: WorkerChannel;
  private analysisWorker: Promise<WorkerChannel> | null = null;

  private constructor(game: WorkerChannel) {
    this.game = game;
  }

  /** Spawns the game worker and loads the wasm module; rejects when the pkg is not available. */
  static async create(): Promise<WasmEngine> {
    const game = new WorkerChannel();
    try {
      await game.call('init');
    } catch (e) {
      game.terminate();
      throw e;
    }
    return new WasmEngine(game);
  }

  /** Second worker with a throwaway session: record-based calls only need a Session instance. */
  private analysis(): Promise<WorkerChannel> {
    if (!this.analysisWorker) {
      this.analysisWorker = (async () => {
        const ch = new WorkerChannel();
        try {
          await ch.call('init');
          await ch.call('newMatch', {}, 0, 0);
        } catch (e) {
          ch.terminate();
          this.analysisWorker = null;
          throw e;
        }
        return ch;
      })();
    }
    return this.analysisWorker;
  }

  newMatch(settings: Settings, seed: Seed = randomSeed()): Promise<MatchState> {
    return this.game.call('newMatch', settings, seed.hi, seed.lo);
  }
  match(): Promise<MatchState> {
    return this.game.call('match');
  }
  view(player: Player): Promise<PlayerView> {
    return this.game.call('view', player);
  }
  legalMoves(): Promise<Move[]> {
    return this.game.call('legalMoves');
  }
  apply(move: Move): Promise<ApplyResponse> {
    return this.game.call('apply', move);
  }
  nextDeal(): Promise<MatchState> {
    return this.game.call('nextDeal');
  }
  aiMove(level: Level, maxNodes: number): Promise<AiMoveResponse> {
    return this.game.call('aiMove', level, maxNodes);
  }
  evaluate(player: Player, maxNodes: number): Promise<EvalResponse> {
    return this.game.call('evaluate', player, maxNodes);
  }
  record(): Promise<GameRecord> {
    return this.game.call('record');
  }
  async analyze(record: GameRecord, player: Player, maxNodes: number): Promise<AnalysisReport> {
    const ch = await this.analysis();
    return ch.call('analyze', record, player, maxNodes);
  }
  viewAt(record: GameRecord, index: number, player: Player): Promise<PlayerView> {
    return this.game.call('viewAt', record, index, player);
  }
  /** Par of the live deal: fetch its record from the game worker, solve it on the analysis worker. */
  async par(maxNodes: number): Promise<ParReport> {
    const record = await this.record();
    return this.parOfRecord(record, maxNodes);
  }
  async parOfRecord(record: GameRecord, maxNodes: number): Promise<ParReport> {
    const ch = await this.analysis();
    return ch.call('parOfRecord', record, maxNodes);
  }
  snapshot(): Promise<string> {
    return this.game.call('snapshot');
  }
  restore(snapshot: string): Promise<MatchState> {
    return this.game.call('restore', snapshot);
  }
  version(): Promise<string> {
    return this.game.call('version');
  }
  dispose(): void {
    this.game.call('dispose').catch(() => {});
    this.game.terminate();
    if (this.analysisWorker) {
      this.analysisWorker.then((ch) => ch.terminate()).catch(() => {});
      this.analysisWorker = null;
    }
  }
}
