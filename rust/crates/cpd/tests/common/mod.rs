//! Fixtures shared by the integration test binaries of this crate. Each
//! binary compiles this module on its own, so unused items are expected.
#![allow(dead_code)]

/// A function and a structurally similar rewrite of it (other names, one
/// extra statement, one extra call): no exact clone, but `--similarity 0.7`
/// matches them. Used by the CLI and the MCP tests.
pub const INVOICE_JS: &str = "export function buildInvoice(order, customer, taxRate) {\n  const lines = [];\n  for (const item of order.items) {\n    const net = item.price * item.quantity;\n    lines.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = lines.reduce((sum, line) => sum + line.net, 0);\n  const tax = Math.round(subtotal * taxRate * 100) / 100;\n  return { number: nextInvoiceNumber(), customer: customer.id, lines, subtotal, tax, total: subtotal + tax };\n}\n";
pub const CREDIT_NOTE_JS: &str = "export function buildCreditNote(refund, account, vatRate) {\n  const entries = [];\n  for (const item of refund.items) {\n    if (!item.refundable) continue;\n    const net = item.price * item.quantity;\n    entries.push({ sku: item.sku, quantity: item.quantity, net });\n  }\n  const subtotal = entries.reduce((sum, entry) => sum + entry.net, 0);\n  const vat = Math.round(subtotal * vatRate * 100) / 100;\n  logger.info('credit note', { account: account.id, subtotal });\n  return { number: nextCreditNoteNumber(), account: account.id, entries, subtotal, vat, total: subtotal + vat };\n}\n";

/// A stand-in embeddings server for `--semantic`, `--compare` and `--lsp`.
///
/// It answers the OpenAI embeddings protocol with bag-of-words vectors:
/// every word of a text (camelCase and snake_case split, so that `unitPrice`
/// and `unit_price` agree) is hashed into one of 256 buckets. Two functions
/// that share their vocabulary point the same way, which is what a code
/// model does for a function and its port to another language.
pub mod embeddings {
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    fn words(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut word = String::new();
        let mut prev_lower = false;
        for c in text.chars() {
            if c.is_ascii_alphanumeric() {
                if c.is_ascii_uppercase() && prev_lower && !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                word.push(c.to_ascii_lowercase());
                prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            } else {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                prev_lower = false;
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
        out
    }

    fn bag_of_words(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; 256];
        for w in words(text) {
            let h = w.bytes().fold(0xcbf2_9ce4_8422_2325u64, |acc, b| {
                (acc ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
            });
            v[(h % 256) as usize] += 1.0;
        }
        v
    }

    /// What the stand-in server saw: one entry per request, the JSON body plus
    /// the Authorization header.
    type Log = Arc<Mutex<Vec<(Value, Option<String>)>>>;

    pub struct Server {
        pub url: String,
        log: Log,
    }

    impl Server {
        pub fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let url = format!("http://{}/v1", listener.local_addr().unwrap());
            let log: Log = Arc::default();
            let seen = log.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    answer(stream, &seen);
                }
            });
            Self { url, log }
        }

        pub fn requests(&self) -> Vec<(Value, Option<String>)> {
            self.log.lock().unwrap().clone()
        }
    }

    fn answer(mut stream: std::net::TcpStream, log: &Log) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        let mut length = 0;
        let mut auth = None;
        reader.read_line(&mut line).unwrap();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').unwrap_or_default();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap(),
                "authorization" => auth = Some(value.trim().to_string()),
                _ => {}
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let request: Value = serde_json::from_slice(&body).unwrap();
        let data: Vec<Value> = request["input"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, text)| json!({"object": "embedding", "index": i, "embedding": bag_of_words(text.as_str().unwrap())}))
            .collect();
        log.lock().unwrap().push((request, auth));
        let payload = json!({"object": "list", "data": data, "model": "stand-in"}).to_string();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
    }

    /// A cart total implemented in a Rust backend and again in a Svelte
    /// component, plus a dozen unrelated functions on each side, under
    /// `root`: the stand-in server pairs the two cart totals.
    pub fn cart_project(root: &std::path::Path) {
        let write = |rel: &str, content: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        };
        write(
            "backend/src/cart.rs",
            "pub fn cart_total(lines: &[Line], coupon_percent: i64) -> i64 {\n    let subtotal: i64 = lines.iter().map(|line| line.unit_price * line.quantity).sum();\n    let discount = subtotal * coupon_percent / 100;\n    let shipping = if subtotal - discount > 5000 { 0 } else { 499 };\n    subtotal - discount + shipping\n}\n",
        );
        write(
            "frontend/src/Cart.svelte",
            "<script lang=\"ts\">\n  let { lines, couponPercent } = $props();\n\n  function cartTotal(items: Line[], percent: number): number {\n    const subtotal = items.reduce((sum, line) => sum + line.unitPrice * line.quantity, 0);\n    const discount = Math.floor((subtotal * percent) / 100);\n    const shipping = subtotal - discount > 5000 ? 0 : 499;\n    return subtotal - discount + shipping;\n  }\n</script>\n\n<p>{cartTotal(lines, couponPercent)}</p>\n",
        );
        // Every filler has words of its own, on both sides.
        const RUST_TOPICS: [&str; 12] = [
            "alpha bravo charlie",
            "delta echo foxtrot",
            "golf hotel india",
            "juliet kilo lima",
            "mike november oscar",
            "papa quebec romeo",
            "sierra tango uniform",
            "victor whiskey xray",
            "yankee zulu amber",
            "basalt cobalt dune",
            "ember fjord glacier",
            "harbor island jungle",
        ];
        const TS_TOPICS: [&str; 12] = [
            "kettle lantern meadow",
            "nectar orchid pebble",
            "quartz raven saddle",
            "timber umber velvet",
            "walnut yarrow zephyr",
            "anchor beacon canyon",
            "dagger falcon gypsum",
            "hazel iris jasper",
            "kelp lotus mango",
            "nutmeg olive pepper",
            "quill rhubarb sorrel",
            "thistle ursa vervain",
        ];
        let mut rust = String::new();
        let mut ts = String::new();
        for k in 0..12 {
            let w: Vec<&str> = RUST_TOPICS[k].split(' ').collect();
            rust.push_str(&format!(
                "pub fn {a}_{k}({b}: u32) -> u32 {{\n    let {c} = {b} + {k};\n    let {a} = {c} * 3;\n    {a} - {b}\n}}\n\n",
                a = w[0], b = w[1], c = w[2]
            ));
            let w: Vec<&str> = TS_TOPICS[k].split(' ').collect();
            ts.push_str(&format!(
                "export function {c}{k}({a}: string): string {{\n  const {b} = {a}.trim();\n  const {c} = {b}.toUpperCase();\n  return {c} + '{k}';\n}}\n\n",
                a = w[0], b = w[1], c = w[2]
            ));
        }
        write("backend/src/misc.rs", &rust);
        write("frontend/src/misc.ts", &ts);
    }
}
