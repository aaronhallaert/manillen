<script lang="ts">
  import { bidLabel, type Bid } from '../engine/types';

  interface Props {
    bid: Bid | null;
    multiplier: number;
    oncontra: (contra: boolean) => void;
  }

  let { bid, multiplier, oncontra }: Props = $props();
</script>

<div class="banner" role="region" aria-label="contra">
  <div class="dialog">
    <h2>Dealer chose {bidLabel(bid)}</h2>
    <p class="sub">Contra doubles the score{multiplier > 1 ? ` (×${multiplier * 2} in total)` : ''}. Only if you expect to win.</p>
    <div class="actions">
      <button type="button" class="btn primary" onclick={() => oncontra(true)}>Contra</button>
      <button type="button" class="btn" onclick={() => oncontra(false)}>Play on</button>
    </div>
  </div>
</div>
