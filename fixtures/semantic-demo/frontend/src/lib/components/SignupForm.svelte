<script lang="ts">
  import { api } from '$lib/api';

  type Strength = 'weak' | 'fair' | 'strong';

  const MAX_EMAIL_LENGTH = 254;
  const BLOCKED_PASSWORDS = new Set(['password', '123456', 'qwerty', 'letmein', 'iloveyou', 'admin']);

  let email = $state('');
  let password = $state('');
  let submitting = $state(false);
  let serverError = $state<string | null>(null);

  // Returns the message to show under the field, or null when the address is fine.
  function checkEmail(value: string): string | null {
    const address = value.trim();
    if (address === '') return 'Please enter your email';
    if (address.length > MAX_EMAIL_LENGTH) return 'That email is too long';
    const at = address.indexOf('@');
    if (at <= 0 || at !== address.lastIndexOf('@') || at > 64) {
      return 'That does not look like an email address';
    }
    const domainParts = address.slice(at + 1).split('.');
    const goodDomain =
      domainParts.length > 1 &&
      domainParts.every((part) => /^[a-z0-9]+(-[a-z0-9]+)*$/i.test(part));
    return goodDomain ? null : 'Check the part after @';
  }

  function rate(pw: string): Strength {
    if (BLOCKED_PASSWORDS.has(pw.toLowerCase())) return 'weak';
    const checks = [
      pw.length >= 8,
      pw.length >= 12,
      /[a-z]/.test(pw) && /[A-Z]/.test(pw),
      /\d/.test(pw),
      /[^A-Za-z0-9]/.test(pw),
    ];
    const passed = checks.filter(Boolean).length;
    if (passed >= 4) return 'strong';
    return passed === 3 ? 'fair' : 'weak';
  }

  const emailError = $derived(email ? checkEmail(email) : null);
  const strength = $derived(rate(password));

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (emailError || strength === 'weak') return;
    submitting = true;
    serverError = null;
    try {
      await api('/api/signup', { method: 'POST', body: JSON.stringify({ email, password }) });
    } catch (err) {
      serverError = err instanceof Error ? err.message : 'Sign-up failed';
    } finally {
      submitting = false;
    }
  }
</script>

<form onsubmit={submit} novalidate>
  <label>
    Email
    <input type="email" bind:value={email} autocomplete="email" />
  </label>
  {#if emailError}<p class="error">{emailError}</p>{/if}

  <label>
    Password
    <input type="password" bind:value={password} autocomplete="new-password" />
  </label>
  <meter min="0" max="2" value={['weak', 'fair', 'strong'].indexOf(strength)}></meter>

  {#if serverError}<p class="error">{serverError}</p>{/if}
  <button disabled={submitting}>Create account</button>
</form>
