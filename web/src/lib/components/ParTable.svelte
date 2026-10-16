<script lang="ts">
  import { bidLabel, sameBid, type Bid, type ParReport, type Player } from '../engine/types';

  interface Props {
    par: ParReport;
    me: Player;
    /** The bid actually made, if any (highlighted). */
    actualBid?: Bid | null;
  }

  let { par, me, actualBid = null }: Props = $props();

  const opp = $derived(me === 0 ? 1 : 0);
  const iAmDealer = $derived(par.dealer === me);

  /** Signed score from the human's seat. */
  function myScore(dealerScore: number): number {
    return iAmDealer ? dealerScore : -dealerScore;
  }
  function signed(n: number): string {
    return n > 0 ? `+${n}` : `${n}`;
  }
</script>

<table class="par" title="Hindsight: both players see every card and play perfectly. Non-dealer leads.">
  <thead>
    <tr><th>Bid</th><th>You</th><th>Opp</th><th>Contra</th><th>Score</th></tr>
  </thead>
  <tbody>
    {#each par.bids as b (bidLabel(b.bid))}
      <tr class:best={sameBid(b.bid, par.best_bid)} class:actual={sameBid(b.bid, actualBid)}>
        <td>
          {bidLabel(b.bid)}
          {#if sameBid(b.bid, par.best_bid)}<span class="tag">best</span>{/if}
          {#if sameBid(b.bid, actualBid)}<span class="tag played">played</span>{/if}
        </td>
        <td>{b.points[me]}</td>
        <td>{b.points[opp]}</td>
        <td>{b.contra ? 'yes' : '–'}</td>
        <td class="score">{signed(myScore(b.dealer_score))}</td>
      </tr>
    {/each}
  </tbody>
</table>
<p class="foot">Score is from your seat ({iAmDealer ? 'you deal' : 'opponent deals'}); par = {signed(myScore(par.par_score))} with the dealer's best bid.</p>

<style>
  .par {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }
  .par th,
  .par td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--line);
    text-align: right;
  }
  .par th:first-child,
  .par td:first-child {
    text-align: left;
  }
  .par th {
    color: var(--muted);
    font-weight: 600;
  }
  tr.best td {
    font-weight: 700;
  }
  tr.actual td {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .tag {
    margin-left: 4px;
    border-radius: 999px;
    padding: 1px 6px;
    font-size: 0.65rem;
    font-weight: 700;
    background: var(--c-best);
    color: #fff;
    vertical-align: middle;
  }
  .tag.played {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .score {
    font-weight: 600;
  }
  .foot {
    margin: 6px 0 0;
    font-size: 0.75rem;
    color: var(--muted);
  }
</style>
