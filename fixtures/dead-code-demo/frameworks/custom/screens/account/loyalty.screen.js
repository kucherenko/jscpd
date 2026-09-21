export default {
  mount(route) {
    const stamps = 7;
    const untilFree = Math.max(10 - stamps, 0);
    return { route, heading: 'Loyalty card', message: `${untilFree} more for a free drink` };
  },
};
