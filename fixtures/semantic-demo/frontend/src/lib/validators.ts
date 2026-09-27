// Checks for the newsletter form in the footer. The sign-up form has its own
// e-mail check; nobody noticed there were two.

const EMAIL_PATTERN = /^[^\s@]{1,64}@([a-z0-9]+(-[a-z0-9]+)*\.)+[a-z0-9]+(-[a-z0-9]+)*$/i;

export function isEmail(value: string): boolean {
  const candidate = value.trim();
  if (candidate.length === 0 || candidate.length > 254) {
    return false;
  }
  if (candidate.split('@').length !== 2) {
    return false;
  }
  return EMAIL_PATTERN.test(candidate);
}

export function newsletterErrors(form: { email: string; consent: boolean }): string[] {
  const errors: string[] = [];
  if (!isEmail(form.email)) {
    errors.push('Enter a valid email address');
  }
  if (!form.consent) {
    errors.push('Please accept the newsletter terms');
  }
  return errors;
}
