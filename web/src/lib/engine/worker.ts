// Web Worker hosting the wasm Session so AI search never blocks the UI.
// Protocol: {id, method, args} -> {id, ok, result | error}.

export interface WorkerRequest {
  id: number;
  method: string;
  args: unknown[];
}

export interface WorkerResponse {
  id: number;
  ok: boolean;
  result?: unknown;
  error?: string;
}

// Shape of the wasm-bindgen Session (docs/api.md). Every method speaks JSON strings.
interface WasmSession {
  match_json(): string;
  view_json(player: number): string;
  legal_moves_json(): string;
  apply_json(move: string): string;
  next_deal(): string;
  ai_move_json(level: string, maxNodes: number): string;
  evaluate_json(player: number, maxNodes: number): string;
  record_json(): string;
  analyze_json(record: string, player: number, maxNodes: number): string;
  view_at_json(record: string, index: number, player: number): string;
  par_json(maxNodes: number): string;
  par_of_record_json(record: string, maxNodes: number): string;
  snapshot_json(): string;
  version(): string;
  free(): void;
}

interface WasmModule {
  default: (input?: unknown) => Promise<unknown>;
  Session: (new (hi: number, lo: number, settings: string) => WasmSession) & {
    restore(snapshot: string): WasmSession;
  };
}

let mod: WasmModule | null = null;
let session: WasmSession | null = null;

async function ensureModule(): Promise<WasmModule> {
  if (mod) return mod;
  const m = (await import('manillen-wasm')) as unknown as WasmModule;
  await m.default();
  mod = m;
  return m;
}

function need(): WasmSession {
  if (!session) throw new Error('no match started');
  return session;
}

const json = (s: string) => JSON.parse(s) as unknown;

const handlers: Record<string, (...args: any[]) => Promise<unknown> | unknown> = {
  async init() {
    await ensureModule();
    return true;
  },
  async newMatch(settings: unknown, hi: number, lo: number) {
    const m = await ensureModule();
    session?.free();
    session = new m.Session(hi >>> 0, lo >>> 0, JSON.stringify(settings));
    return json(session.match_json());
  },
  match: () => json(need().match_json()),
  view: (player: number) => json(need().view_json(player)),
  legalMoves: () => json(need().legal_moves_json()),
  apply: (move: unknown) => json(need().apply_json(JSON.stringify(move))),
  nextDeal: () => json(need().next_deal()),
  aiMove: (level: string, maxNodes: number) => json(need().ai_move_json(level, maxNodes)),
  evaluate: (player: number, maxNodes: number) => json(need().evaluate_json(player, maxNodes)),
  record: () => json(need().record_json()),
  analyze: (record: unknown, player: number, maxNodes: number) =>
    json(need().analyze_json(JSON.stringify(record), player, maxNodes)),
  viewAt: (record: unknown, index: number, player: number) =>
    json(need().view_at_json(JSON.stringify(record), index, player)),
  par: (maxNodes: number) => json(need().par_json(maxNodes)),
  parOfRecord: (record: unknown, maxNodes: number) => json(need().par_of_record_json(JSON.stringify(record), maxNodes)),
  snapshot: () => need().snapshot_json(),
  async restore(snapshot: string) {
    const m = await ensureModule();
    const next = m.Session.restore(snapshot);
    session?.free();
    session = next;
    return json(session.match_json());
  },
  version: () => need().version(),
  dispose() {
    session?.free();
    session = null;
    return true;
  },
};

self.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  const { id, method, args } = ev.data;
  const reply = (r: WorkerResponse) => (self as unknown as Worker).postMessage(r);
  const h = handlers[method];
  if (!h) return reply({ id, ok: false, error: `unknown method ${method}` });
  try {
    reply({ id, ok: true, result: await h(...args) });
  } catch (e) {
    reply({ id, ok: false, error: e instanceof Error ? e.message : String(e) });
  }
};
