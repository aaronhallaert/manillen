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

/** Engine backed by the wasm Session running inside a Web Worker. */
export class WasmEngine implements Engine {
  readonly kind = 'wasm' as const;
  private worker: Worker;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();

  private constructor(worker: Worker) {
    this.worker = worker;
    worker.onmessage = (ev: MessageEvent<WorkerResponse>) => {
      const p = this.pending.get(ev.data.id);
      if (!p) return;
      this.pending.delete(ev.data.id);
      if (ev.data.ok) p.resolve(ev.data.result);
      else p.reject(new Error(ev.data.error ?? 'engine error'));
    };
    worker.onerror = (ev) => {
      const err = new Error(ev.message || 'worker error');
      for (const p of this.pending.values()) p.reject(err);
      this.pending.clear();
    };
  }

  /** Spawns the worker and loads the wasm module; rejects when the pkg is not available. */
  static async create(): Promise<WasmEngine> {
    const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    const engine = new WasmEngine(worker);
    try {
      await engine.call('init');
    } catch (e) {
      worker.terminate();
      throw e;
    }
    return engine;
  }

  private call<T>(method: string, ...args: unknown[]): Promise<T> {
    const id = this.nextId++;
    // Svelte `$state` proxies cannot be structured-cloned; everything we send is plain JSON.
    const req: WorkerRequest = { id, method, args: JSON.parse(JSON.stringify(args)) };
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.worker.postMessage(req);
    });
  }

  newMatch(settings: Settings, seed: Seed = randomSeed()): Promise<MatchState> {
    return this.call('newMatch', settings, seed.hi, seed.lo);
  }
  match(): Promise<MatchState> {
    return this.call('match');
  }
  view(player: Player): Promise<PlayerView> {
    return this.call('view', player);
  }
  legalMoves(): Promise<Move[]> {
    return this.call('legalMoves');
  }
  apply(move: Move): Promise<ApplyResponse> {
    return this.call('apply', move);
  }
  nextDeal(): Promise<MatchState> {
    return this.call('nextDeal');
  }
  aiMove(level: Level, maxNodes: number): Promise<AiMoveResponse> {
    return this.call('aiMove', level, maxNodes);
  }
  evaluate(player: Player, maxNodes: number): Promise<EvalResponse> {
    return this.call('evaluate', player, maxNodes);
  }
  record(): Promise<GameRecord> {
    return this.call('record');
  }
  analyze(record: GameRecord, player: Player, maxNodes: number): Promise<AnalysisReport> {
    return this.call('analyze', record, player, maxNodes);
  }
  viewAt(record: GameRecord, index: number, player: Player): Promise<PlayerView> {
    return this.call('viewAt', record, index, player);
  }
  par(maxNodes: number): Promise<ParReport> {
    return this.call('par', maxNodes);
  }
  parOfRecord(record: GameRecord, maxNodes: number): Promise<ParReport> {
    return this.call('parOfRecord', record, maxNodes);
  }
  snapshot(): Promise<string> {
    return this.call('snapshot');
  }
  restore(snapshot: string): Promise<MatchState> {
    return this.call('restore', snapshot);
  }
  version(): Promise<string> {
    return this.call('version');
  }
  dispose(): void {
    this.call('dispose').catch(() => {});
    this.worker.terminate();
  }
}
