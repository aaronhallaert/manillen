<script lang="ts">
  import ParTable from './ParTable.svelte';
  import { bidLabel, sameBid, type DealResult, type MatchState, type ParReport, type Player } from '../engine/types';

  interface Props {
    result: DealResult;
    me: Player;
    match: MatchState | null;
    par?: ParReport | null;
    parPending?: boolean;
    onreview: () => void;
    onnext: () => void;
    onlobby: () => void;
  }

  let { result, me, match, par = null, parPending = false, onreview, onnext, onlobby }: Props = $props();

  const parSentence = $derived.by(() => {
    if (!par) return null;
    const iAmDealer = par.dealer === me;
    const sign = (n: number) => (n > 0 ? `+${n}` : `${n}`);
    const mine = result.winner === null ? 0 : result.winner === me ? result.score : -result.score;
    const parMine = iAmDealer ? par.par_score : -par.par_score;
    const row = par.actual ?? par.bids.find((b) => sameBid(b.bid, result.bid)) ?? null;
    const first = `You scored ${sign(mine)}; par was ${sign(parMine)} for you (${iAmDealer ? 'your' : "dealer's"} best bid ${bidLabel(par.best_bid)}).`;
    const second = row ? ` Perfect play on ${bidLabel(row.bid)} would give you ${row.points[me]} points; you took ${result.points[me]}.` : '';
    return first + second;
  });

  const opp = $derived(me === 0 ? 1 : 0);
  const outcome = $derived(result.winner === null ? 'Tie — nobody scores' : result.winner === me ? 'You win the deal' : 'Opponent wins the deal');
  const matchOver = $derived(match?.finished ?? false);
</script>

<div class="overlay" role="dialog" aria-modal="true" aria-label="deal result">
  <div class="dialog">
    <h2>{outcome}</h2>
    <table class="result">
      <tbody>
        <tr><td>Card points</td><td>you {result.points[me]} · opp {result.points[opp]}</td></tr>
        <tr><td>Bid</td><td>{bidLabel(result.bid)}{result.contra ? ' + contra' : ''}</td></tr>
        <tr><td>Base</td><td>{result.base} (points above 30)</td></tr>
        <tr><td>Multiplier</td><td>×{result.multiplier}</td></tr>
        <tr class="total"><td>Deal score</td><td>{result.winner === null ? 0 : result.winner === me ? '+' : '−'}{result.score}</td></tr>
        {#if match}
          <tr><td>Match</td><td>you {match.scores[me]} · opp {match.scores[opp]} (to {match.target})</td></tr>
        {/if}
      </tbody>
    </table>
    {#if matchOver && match}
      <p class="sub"><strong>{match.winner === me ? 'You won the match!' : 'Opponent won the match.'}</strong></p>
    {/if}
    <details class="parbox" open={par !== null}>
      <summary>Par (perfect information)</summary>
      {#if par}
        <p class="sub">{parSentence}</p>
        <ParTable {par} {me} actualBid={result.bid} />
      {:else if parPending}
        <p class="sub">computing par…</p>
      {:else}
        <p class="sub">not available</p>
      {/if}
    </details>
    <div class="actions">
      <button type="button" class="btn primary" onclick={onreview}>Review this deal</button>
      {#if matchOver}
        <button type="button" class="btn" onclick={onlobby}>Back to lobby</button>
      {:else}
        <button type="button" class="btn" onclick={onnext}>Next deal</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .result {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
    margin: 8px 0 12px;
  }
  .result td {
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .result td:last-child {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .total td {
    font-weight: 700;
    font-size: 1rem;
  }
  .parbox {
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 6px 10px;
    margin-bottom: 8px;
  }
  .parbox summary {
    cursor: pointer;
    font-weight: 600;
    font-size: 0.9rem;
    min-height: 32px;
    display: flex;
    align-items: center;
  }
</style>
