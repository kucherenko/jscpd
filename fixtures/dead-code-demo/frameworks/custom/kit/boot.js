import { mountScreens } from './mount.js';

const started = await mountScreens(new URL('../screens/', import.meta.url));
console.log(`kiosk ready with ${started} screens`);
