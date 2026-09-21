// The Zebra printers went back to the lessor in March.
const DOTS_PER_MM = 8;

export function toZpl(sku, widthMm) {
  const width = Math.round(widthMm * DOTS_PER_MM);
  return `^XA^PW${width}^FO40,40^A0N,48,48^FD${sku}^FS^XZ`;
}
