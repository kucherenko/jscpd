import { openPage } from '@/router.js';
import { translate } from '@/i18n.js';

const target = document.querySelector('#app');
openPage(location.hash.slice(1) || 'home').then((page) => {
  target.dataset.title = translate(page.title);
});
