const FREE_DRINK_AT = 10;

// The router calls this before mounting: no file in the project does.
export function guard(session) {
  return Boolean(session?.memberId);
}

// Left behind when the spring promotion ended.
export function legacyPromoCode(stamps) {
  return stamps >= FREE_DRINK_AT ? 'SPRING-FREE' : null;
}

export default {
  mount(route) {
    const stamps = 7;
    const untilFree = Math.max(FREE_DRINK_AT - stamps, 0);
    return { route, heading: 'Loyalty card', message: `${untilFree} more for a free drink` };
  },
};
