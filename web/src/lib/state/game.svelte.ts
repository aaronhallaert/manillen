import type { Engine, Seed } from '../engine/client';
import { createEngine } from '../engine';
import type {
  AnalysisReport,
  DealResult,
  Event,
  GameRecord,
  MatchState,
  Move,
  ParReport,
  Player,
  PlayerView,
  Settings,
  Trick,
} from '../engine/types';
import { DEFAULT_SETTINGS } from '../engine/types';

export type Screen = 'lobby' | 'table' | 'review';

const LS_SETTINGS = 'manillen.settings';
const LS_RECORD = 'manillen.lastRecord';
const LS_SNAPSHOT = 'manillen.snapshot';

/** How long a completed trick stays on the table before the engine leads the next one. */
const TRICK_PAUSE_MS = 1400;
/** Minimum time the engine appears to think, so the human's card is seen before the reply. */
const MIN_AI_REPLY_MS = 600;

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

function loadString(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function saveString(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: ignore */
  }
}

function loadJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function saveJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage unavailable: ignore */
  }
}

export class GameStore {
  engine: Engine | null = $state(null);
  engineKind: 'wasm' | 'mock' | null = $state(null);
  settings: Settings = $state({ ...DEFAULT_SETTINGS });
  screen: Screen = $state('lobby');

  matchState: MatchState | null = $state(null);
  view: PlayerView | null = $state(null);
  lastEvents: Event[] = $state([]);
  lastDealResult: DealResult | null = $state(null);
  aiThinking = $state(false);
  /** Trick that just completed; shown in the centre until the next card is led. */
  shownTrick: Trick | null = $state(null);
  busy = $state(false);
  error: string | null = $state(null);
  seed: Seed | null = $state(null);

  /** Record of the current (or most recently finished) deal. */
  record: GameRecord | null = $state(null);
  /** Record loaded for review (may come from a file). */
  reviewRecord: GameRecord | null = $state(null);
  reviewPlayer: Player = $state(0);
  analysis: AnalysisReport | null = $state(null);
  pendingAnalysis = $state(false);

  /** Hindsight par of the current deal (only fetched when show_par is on or the deal ended). */
  par: ParReport | null = $state(null);
  parPending = $state(false);
  /** Par of the record under review. */
  reviewPar: ParReport | null = $state(null);
  reviewParPending = $state(false);
  private parCache = new Map<string, ParReport>();

  /** Snapshot saved by a previous page session (resume after reload). */
  savedSnapshot: string | null = $state(null);

  legalMoves: Move[] = $derived(this.view?.legal_moves ?? []);
  isMyTurn: boolean = $derived(this.view !== null && this.view.to_move === this.settings.human);
  hasMatch: boolean = $derived(this.matchState !== null && this.view !== null);
  canContinue: boolean = $derived(this.hasMatch || this.savedSnapshot !== null);

  constructor() {
    const saved = loadJson<Settings>(LS_SETTINGS);
    if (saved) this.settings = { ...DEFAULT_SETTINGS, ...saved };
    this.record = loadJson<GameRecord>(LS_RECORD);
    this.savedSnapshot = loadString(LS_SNAPSHOT);
  }

  /** Attach an engine. Called once at app start; tests inject a MockEngine. */
  async init(engine?: Engine): Promise<void> {
    this.engine = engine ?? (await createEngine());
    this.engineKind = this.engine.kind;
  }

  updateSettings(patch: Partial<Settings>): void {
    this.settings = { ...this.settings, ...patch };
    saveJson(LS_SETTINGS, this.settings);
  }

  async newMatch(seed?: Seed): Promise<void> {
    const engine = this.need();
    await this.guard(async () => {
      this.seed = seed ?? null;
      this.matchState = await engine.newMatch(this.settings, seed);
      this.lastDealResult = null;
      this.analysis = null;
      this.lastEvents = [];
      this.par = null;
      this.shownTrick = null;
      await this.refresh();
      await this.saveSnapshot();
      this.screen = 'table';
    });
    this.startDealPar();
    await this.runAiIfNeeded();
  }

  /** Resume the live match, or restore the one saved before the last reload. */
  async continueMatch(): Promise<void> {
    if (this.hasMatch) {
      this.screen = 'table';
      await this.runAiIfNeeded();
      return;
    }
    const snapshot = this.savedSnapshot;
    const engine = this.need();
    if (!snapshot) return;
    await this.guard(async () => {
      this.matchState = await engine.restore(snapshot);
      try {
        const snap = JSON.parse(snapshot) as { settings?: Partial<Settings> };
        if (snap.settings) this.settings = { ...this.settings, ...snap.settings };
      } catch {
        /* keep current settings */
      }
      this.lastDealResult = null;
      this.analysis = null;
      this.lastEvents = [];
      this.par = null;
      this.shownTrick = null;
      await this.refresh();
      if (this.view?.phase === 'done') {
        const deals = this.matchState?.deals ?? [];
        this.lastDealResult = deals[deals.length - 1] ?? null;
      }
      this.screen = 'table';
    });
    this.startDealPar();
    await this.runAiIfNeeded();
  }

  /** Human move. */
  async play(move: Move): Promise<void> {
    const engine = this.need();
    if (!this.isMyTurn) return;
    await this.guard(async () => {
      await this.applyMove(engine, move);
    });
    await this.runAiIfNeeded();
  }

  /** Lets the engine move for the non-human seat until it is the human's turn or the deal ends. */
  async runAiIfNeeded(): Promise<void> {
    const engine = this.engine;
    if (!engine) return;
    while (this.view && this.view.phase !== 'done' && this.view.to_move !== null && this.view.to_move !== this.settings.human) {
      if (this.shownTrick && this.view.phase === 'play') await sleep(TRICK_PAUSE_MS);
      this.aiThinking = true;
      try {
        const started = Date.now();
        const { move } = await engine.aiMove(this.settings.level, this.settings.max_nodes);
        if (move.type === 'play') {
          const wait = MIN_AI_REPLY_MS - (Date.now() - started);
          if (wait > 0) await sleep(wait);
        }
        await this.applyMove(engine, move);
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        break;
      } finally {
        this.aiThinking = false;
      }
    }
  }

  async nextDeal(): Promise<void> {
    const engine = this.need();
    await this.guard(async () => {
      this.matchState = await engine.nextDeal();
      this.lastDealResult = null;
      this.analysis = null;
      this.lastEvents = [];
      this.par = null;
      this.shownTrick = null;
      await this.refresh();
      await this.saveSnapshot();
      this.screen = 'table';
    });
    this.startDealPar();
    await this.runAiIfNeeded();
  }

  async analyzeCurrentDeal(): Promise<void> {
    const engine = this.need();
    const record = this.record ?? (await engine.record());
    await this.openReview(record, this.settings.human);
  }

  /** Analyze any record (current deal or one loaded from a file) and show the review screen. */
  async openReview(record: GameRecord, player: Player): Promise<void> {
    const engine = this.need();
    this.reviewRecord = record;
    this.reviewPlayer = player;
    this.analysis = null;
    this.screen = 'review';
    void this.loadReviewPar(record);
    this.pendingAnalysis = true;
    try {
      this.analysis = await engine.analyze(record, player, this.settings.max_nodes);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.pendingAnalysis = false;
    }
  }

  async viewAt(index: number): Promise<PlayerView | null> {
    const engine = this.engine;
    if (!engine || !this.reviewRecord) return null;
    try {
      return await engine.viewAt(this.reviewRecord, index, this.reviewPlayer);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  goTo(screen: Screen): void {
    this.screen = screen;
  }

  clearError(): void {
    this.error = null;
  }

  // ---- internals ----

  private need(): Engine {
    if (!this.engine) throw new Error('engine not initialised');
    return this.engine;
  }

  private async guard(fn: () => Promise<void>): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await fn();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.busy = false;
    }
  }

  private async applyMove(engine: Engine, move: Move): Promise<void> {
    const res = await engine.apply(move);
    this.lastEvents = res.events;
    if (move.type === 'play') this.shownTrick = null;
    for (const ev of res.events) {
      if (ev.type === 'deal_over') this.lastDealResult = ev.result;
      if (ev.type === 'trick') this.shownTrick = ev.trick;
    }
    this.view = res.view;
    this.record = await engine.record();
    saveJson(LS_RECORD, this.record);
    if (res.events.some((ev) => ev.type === 'deal_over' || ev.type === 'match_over')) {
      this.matchState = await engine.match();
    }
    await this.saveSnapshot();
  }

  /** Fire the par computation for a freshly dealt deal when the player asked for it. */
  private startDealPar(): void {
    if (this.settings.show_par) void this.fetchPar();
  }

  /** Fetch the current deal's par once; later calls while loaded or pending are no-ops. */
  async fetchPar(): Promise<void> {
    const engine = this.engine;
    if (!engine || this.par || this.parPending) return;
    this.parPending = true;
    try {
      this.par = await engine.par(0);
    } catch (e) {
      console.warn('[manillen] par failed:', e instanceof Error ? e.message : e);
    } finally {
      this.parPending = false;
    }
  }

  private async loadReviewPar(record: GameRecord): Promise<void> {
    const engine = this.engine;
    if (!engine) return;
    const key = JSON.stringify(record.layout) + record.dealer;
    const cached = this.parCache.get(key);
    if (cached) {
      this.reviewPar = cached;
      return;
    }
    this.reviewPar = null;
    this.reviewParPending = true;
    try {
      const par = await engine.parOfRecord(record, 0);
      this.parCache.set(key, par);
      this.reviewPar = par;
    } catch (e) {
      console.warn('[manillen] par failed:', e instanceof Error ? e.message : e);
    } finally {
      this.reviewParPending = false;
    }
  }

  private async saveSnapshot(): Promise<void> {
    const engine = this.engine;
    if (!engine) return;
    try {
      const snap = await engine.snapshot();
      this.savedSnapshot = snap;
      saveString(LS_SNAPSHOT, snap);
    } catch {
      /* snapshot unsupported or storage unavailable */
    }
  }

  private async refresh(): Promise<void> {
    const engine = this.need();
    this.view = await engine.view(this.settings.human);
    this.record = await engine.record();
    saveJson(LS_RECORD, this.record);
  }
}

export const game = new GameStore();
