<script lang="ts">
  import type { CartLine, Coupon } from '$lib/types';

  interface Totals {
    subtotal: number;
    discount: number;
    tax: number;
    shipping: number;
    total: number;
  }

  let { lines, coupon = null }: { lines: CartLine[]; coupon?: Coupon | null } = $props();

  const VAT_PERCENT = 20;
  const FREE_DELIVERY_OVER = 5000;
  const DELIVERY_COST = 499;

  // Mirrors what checkout will charge, so the summary never surprises anyone.
  function computeTotals(items: CartLine[], code: Coupon | null): Totals {
    const subtotal = items.reduce((sum, item) => sum + item.unitPriceCents * item.quantity, 0);
    let discount = 0;
    if (code?.kind === 'percent') {
      discount = Math.floor((subtotal * Math.min(code.percent, 100)) / 100);
    } else if (code?.kind === 'fixed') {
      discount = Math.max(0, Math.min(code.amountCents, subtotal));
    }
    const afterDiscount = subtotal - discount;
    const tax = Math.round((afterDiscount * VAT_PERCENT) / 100);
    const shipping = items.length === 0 || afterDiscount >= FREE_DELIVERY_OVER ? 0 : DELIVERY_COST;
    return { subtotal, discount, tax, shipping, total: afterDiscount + tax + shipping };
  }

  const money = new Intl.NumberFormat('en-GB', { style: 'currency', currency: 'EUR' });
  const totals = $derived(computeTotals(lines, coupon));
</script>

<dl class="summary">
  <dt>Subtotal</dt>
  <dd>{money.format(totals.subtotal / 100)}</dd>
  {#if totals.discount > 0}
    <dt>Discount</dt>
    <dd>−{money.format(totals.discount / 100)}</dd>
  {/if}
  <dt>VAT</dt>
  <dd>{money.format(totals.tax / 100)}</dd>
  <dt>Shipping</dt>
  <dd>{totals.shipping === 0 ? 'Free' : money.format(totals.shipping / 100)}</dd>
  <dt class="total">Total</dt>
  <dd class="total">{money.format(totals.total / 100)}</dd>
</dl>
