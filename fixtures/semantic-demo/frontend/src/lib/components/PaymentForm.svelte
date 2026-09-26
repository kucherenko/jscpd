<script lang="ts">
  let cardNumber = $state('');
  let expiry = $state('');

  // Doubled digit -> sum of its digits, precomputed for 0..9.
  const DOUBLED = [0, 2, 4, 6, 8, 1, 3, 5, 7, 9];

  function cardLooksValid(raw: string): boolean {
    const cleaned = raw.replace(/[\s-]/g, '');
    if (!/^\d{12,19}$/.test(cleaned)) {
      return false;
    }
    let total = 0;
    let double = false;
    let i = cleaned.length;
    while (i-- > 0) {
      const digit = cleaned.charCodeAt(i) - 48;
      total += double ? DOUBLED[digit] : digit;
      double = !double;
    }
    return total % 10 === 0;
  }

  function formatExpiry(value: string): string {
    const digits = value.replace(/\D/g, '').slice(0, 4);
    return digits.length > 2 ? `${digits.slice(0, 2)}/${digits.slice(2)}` : digits;
  }

  const cardOk = $derived(cardNumber === '' || cardLooksValid(cardNumber));
</script>

<fieldset>
  <legend>Card</legend>
  <input inputmode="numeric" placeholder="1234 5678 9012 3456" bind:value={cardNumber} />
  {#if !cardOk}<p class="error">Check the card number</p>{/if}
  <input
    inputmode="numeric"
    placeholder="MM/YY"
    value={expiry}
    oninput={(e) => (expiry = formatExpiry(e.currentTarget.value))}
  />
</fieldset>
