import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { transform } from "esbuild";
import { chromium, type Browser, type Page } from "playwright-core";

const ROOT = join(import.meta.dir, "..");
const REPO_ROOT = join(ROOT, "../..");
const STATE_DIR = join(ROOT, "state");
const STATE_FILE = join(STATE_DIR, "stickers.json");
const WIDTH = 960;
const HEIGHT = 540;
const FPS = 12;
const HOST = Bun.env.HOST ?? "127.0.0.1";
const PORT = Number(Bun.env.PORT ?? 8091);
const UDP_HOST = Bun.env.UDP_HOST ?? "127.0.0.1";
const UDP_PORT = Number(Bun.env.UDP_PORT ?? 5001);
const CHROME_PATH = Bun.env.CHROME_PATH ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const CHROMIUM_ARGS = (Bun.env.CHROMIUM_ARGS ?? "").split(/\s+/).filter(Boolean);
const FRAME_INTERVAL = 1000 / FPS;

type Sticker = {
  id: number;
  tsx: string;
  sourceHash: string;
  editableProps: Record<string, EditablePropDefinition>;
  state: Record<string, unknown>;
  schemaRegistered: boolean;
  x: number;
  y: number;
  angle: number;
  createdAt: number;
  updatedAt: number;
};

type EditablePropDefinition = {
  type: "string" | "number" | "boolean" | "object" | "array";
  default: unknown;
};

const stickers = new Map<number, Sticker>();
let nextId = 1;
let browser: Browser | undefined;
let capturePage: Page | undefined;
let ffmpeg: ReturnType<typeof Bun.spawn> | undefined;
let stopping = false;
let saveSequence = 0;
let saveQueue: Promise<void> = Promise.resolve();

const json = (body: unknown, status = 200) => Response.json(body, { status });
const error = (message: string, status = 400) => json({ error: message }, status);
const isObject = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

function sourceHash(source: string) {
  return Bun.hash(source).toString(16);
}

async function saveState() {
  const write = saveQueue.then(async () => {
    await mkdir(STATE_DIR, { recursive: true });
    const temp = `${STATE_FILE}.${process.pid}.${++saveSequence}.tmp`;
    await writeFile(temp, JSON.stringify({ version: 3, stickers: [...stickers.values()] }, null, 2));
    await rename(temp, STATE_FILE);
  });
  saveQueue = write.catch(() => {});
  await write;
}

async function loadState() {
  try {
    const stored = JSON.parse(await readFile(STATE_FILE, "utf8"));
    if (![1, 2, 3].includes(stored.version) || !Array.isArray(stored.stickers)) {
      throw new Error("Unsupported spike sticker state format");
    }
    for (const storedSticker of stored.stickers as Array<Sticker & {
      props?: Record<string, unknown>;
      schemaRegistered?: boolean;
    }>) {
      const hasSchema = Object.prototype.hasOwnProperty.call(storedSticker, "editableProps");
      const sticker: Sticker = {
        ...storedSticker,
        editableProps: storedSticker.editableProps ?? {},
        state: storedSticker.state ?? storedSticker.props ?? {},
        schemaRegistered: storedSticker.schemaRegistered ?? hasSchema,
      };
      delete (sticker as Sticker & { props?: Record<string, unknown> }).props;
      stickers.set(sticker.id, sticker);
      nextId = Math.max(nextId, sticker.id + 1);
    }
    if (stored.version !== 3) await saveState();
  } catch (cause) {
    if ((cause as NodeJS.ErrnoException).code !== "ENOENT") throw cause;
  }
}

async function compileTsx(tsx: string) {
  if (Buffer.byteLength(tsx, "utf8") > 64 * 1024) throw new Error("tsx must be 64 KiB or smaller");
  if (!/\bexport\s+default\b/.test(tsx)) throw new Error("tsx must have a default-exported React component");
  const imports = [...tsx.matchAll(/\b(?:from\s*|import\s*)["']([^"']+)["']/g)].map((match) => match[1]);
  const unsupported = imports.filter((name) => ![
    "react",
    "react/jsx-runtime",
    "@bronzecast/sticker-runtime",
  ].includes(name));
  if (unsupported.length) throw new Error(`Only React imports are supported in this spike: ${unsupported.join(", ")}`);
  return (await transform(tsx, {
    loader: "tsx",
    format: "esm",
    target: "es2022",
    jsx: "automatic",
    jsxImportSource: "react",
  })).code;
}

function normalizeCreate(input: unknown): Pick<Sticker, "tsx" | "x" | "y" | "angle"> & { initialState: Record<string, unknown> } {
  if (!isObject(input)) throw new Error("body must be a JSON object");
  const unsupported = Object.keys(input).filter((key) => !["tsx", "x", "y", "angle", "initialState"].includes(key));
  if (unsupported.length) throw new Error(`POST cannot set: ${unsupported.join(", ")}; provide editable values as initialState`);
  if (typeof input.tsx !== "string") throw new Error("tsx must be a string containing a default-exported component");
  const initialState = input.initialState ?? {};
  if (!isObject(initialState)) throw new Error("initialState must be a JSON object");
  const x = input.x ?? 0;
  const y = input.y ?? 0;
  const angle = input.angle ?? 0;
  if (typeof x !== "number" || !Number.isFinite(x) || x < 0 || x > WIDTH) throw new Error(`x must be between 0 and ${WIDTH}`);
  if (typeof y !== "number" || !Number.isFinite(y) || y < 0 || y > HEIGHT) throw new Error(`y must be between 0 and ${HEIGHT}`);
  if (typeof angle !== "number" || !Number.isFinite(angle)) throw new Error("angle must be a finite number");
  return { tsx: input.tsx, x, y, angle, initialState };
}

function validValue(type: EditablePropDefinition["type"], value: unknown) {
  if (type === "string") return typeof value === "string";
  if (type === "number") return typeof value === "number" && Number.isFinite(value);
  if (type === "boolean") return typeof value === "boolean";
  if (type === "array") return Array.isArray(value);
  return isObject(value);
}

function normalizeEditableProps(input: unknown): Record<string, EditablePropDefinition> {
  if (!isObject(input)) throw new Error("editableProps must be a JSON object");
  const normalized: Record<string, EditablePropDefinition> = {};
  for (const [name, definition] of Object.entries(input)) {
    if (!/^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(name)) throw new Error(`invalid editable prop name: ${name}`);
    if (!isObject(definition)) throw new Error(`editable prop '${name}' must define type and default`);
    const type = definition.type;
    if (!["string", "number", "boolean", "object", "array"].includes(String(type))) {
      throw new Error(`editable prop '${name}' has unsupported type`);
    }
    if (!("default" in definition) || !validValue(type as EditablePropDefinition["type"], definition.default)) {
      throw new Error(`editable prop '${name}' needs a default matching its type`);
    }
    normalized[name] = { type: type as EditablePropDefinition["type"], default: definition.default };
  }
  return normalized;
}

function validateWholeState(state: unknown, schema: Record<string, EditablePropDefinition>) {
  if (!isObject(state)) throw new Error("state must be a JSON object");
  const stateKeys = Object.keys(state).sort();
  const schemaKeys = Object.keys(schema).sort();
  if (JSON.stringify(stateKeys) !== JSON.stringify(schemaKeys)) {
    throw new Error(`state must contain exactly these keys: ${schemaKeys.join(", ") || "(none)"}`);
  }
  for (const [name, definition] of Object.entries(schema)) {
    if (!validValue(definition.type, state[name])) throw new Error(`state.${name} must be a ${definition.type}`);
  }
  return state;
}

async function vendorBundle(entrypoint: string, external: string[] = []) {
  const result = await Bun.build({
    entrypoints: [join(ROOT, entrypoint)],
    target: "browser",
    format: "esm",
    bundle: true,
    external,
    write: false,
  });
  if (!result.success) throw new Error(result.logs.map((log) => log.message).join("\n"));
  const file = result.outputs.find((output) => basename(output.path).endsWith(".js"));
  if (!file) throw new Error(`Could not build ${entrypoint}`);
  return file.text();
}

const runtime = await vendorBundle("src/vendor/runtime.ts");

const clientBuild = await Bun.build({
  entrypoints: [join(ROOT, "src/client.tsx")],
  target: "browser",
  format: "esm",
  bundle: true,
  external: ["react", "react-dom/client", "react/jsx-runtime"],
  write: false,
});
if (!clientBuild.success) throw new Error(clientBuild.logs.map((log) => log.message).join("\n"));
const clientJs = await clientBuild.outputs[0].text();

const html = `<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Chromium sticker spike</title>
<style>
*{box-sizing:border-box}html,body,#root{margin:0;width:${WIDTH}px;height:${HEIGHT}px;overflow:hidden;background:#000}
.board{position:relative;width:${WIDTH}px;height:${HEIGHT}px;overflow:hidden;background:#000}
.mog-canvas{position:absolute;inset:0;width:${WIDTH}px;height:${HEIGHT}px;display:block;z-index:0}
.stickers{position:absolute;inset:0;z-index:2;pointer-events:none}
.sticker{position:absolute;transform-origin:center center;will-change:transform}
.error{max-width:420px;max-height:180px;overflow:hidden;padding:8px;color:#fff;background:#8b1616;font:12px monospace}
.api-error{position:absolute;left:8px;bottom:8px;color:#ff8c8c;font:12px monospace}
</style>
<script type="importmap">{"imports":{"react":"/vendor/runtime.js","react-dom/client":"/vendor/runtime.js","react/jsx-runtime":"/vendor/runtime.js","react/jsx-dev-runtime":"/vendor/runtime.js","@bronzecast/sticker-runtime":"/vendor/runtime.js"}}</script>
</head><body><div id="root"></div><script type="module" src="/app.js"></script></body></html>`;

function stickerRoute(path: string) {
  const match = path.match(/^\/api\/stickers\/(\d+)(?:\/(component\.js|schema|state))?$/);
  if (!match) return null;
  return { id: Number(match[1]), resource: match[2] ?? "sticker" };
}

const server = Bun.serve({
  hostname: HOST,
  port: PORT,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/") {
      return new Response(html, { headers: { "Content-Type": "text/html; charset=utf-8" } });
    }
    if (request.method === "GET" && url.pathname === "/favicon.ico") {
      return new Response(null, { status: 204 });
    }
    if (request.method === "GET" && url.pathname === "/app.js") {
      return new Response(clientJs, { headers: { "Content-Type": "text/javascript; charset=utf-8" } });
    }
    const assetMatch = url.pathname.match(/^\/assets\/mog\/([A-Za-z0-9_.-]+)$/);
    if (request.method === "GET" && assetMatch) {
      const asset = Bun.file(join(REPO_ROOT, "characters", "mog", assetMatch[1]));
      if (!(await asset.exists())) return error("asset not found", 404);
      const mime = assetMatch[1].endsWith(".png") ? "image/png" : "text/plain; charset=utf-8";
      return new Response(asset, { headers: { "Content-Type": mime, "Cache-Control": "public, max-age=3600" } });
    }
    if (request.method === "GET" && url.pathname.startsWith("/vendor/")) {
      return url.pathname === "/vendor/runtime.js"
        ? new Response(runtime, { headers: { "Content-Type": "text/javascript; charset=utf-8" } })
        : error("not found", 404);
    }
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ status: "ok", service: "chromium-sticker-spike", fps: FPS, width: WIDTH, height: HEIGHT, stickers: stickers.size });
    }
    if (request.method === "GET" && url.pathname === "/api/stickers") {
      return json([...stickers.values()]);
    }
    const route = stickerRoute(url.pathname);
    if (route?.resource === "component.js" && request.method === "GET") {
      const sticker = stickers.get(route.id);
      if (!sticker) return error("sticker not found", 404);
      try {
        const code = await compileTsx(sticker.tsx);
        return new Response(code, {
          headers: {
            "Content-Type": "text/javascript; charset=utf-8",
            "Cache-Control": "no-store",
          },
        });
      } catch (cause) {
        return error(cause instanceof Error ? cause.message : String(cause), 422);
      }
    }
    if (route?.resource === "sticker" && request.method === "GET") {
      const sticker = stickers.get(route.id);
      return sticker ? json(sticker) : error("sticker not found", 404);
    }
    if ((route?.resource === "schema" || route?.resource === "state") && request.method === "GET") {
      const sticker = stickers.get(route.id);
      if (!sticker) return error("sticker not found", 404);
      if (!sticker.schemaRegistered) return error("component schema is not registered yet", 409);
      return route.resource === "schema"
        ? json({ editableProps: sticker.editableProps })
        : json(sticker.state);
    }
    if (route?.resource === "schema" && request.method === "PUT") {
      const current = stickers.get(route.id);
      if (!current) return error("sticker not found", 404);
      try {
        const body = await request.json();
        if (!isObject(body)) throw new Error("body must be a JSON object");
        const editableProps = normalizeEditableProps(body.editableProps ?? {});
        const unsupportedState = Object.keys(current.state).filter((name) => !(name in editableProps));
        if (!current.schemaRegistered && unsupportedState.length) {
          throw new Error(`initialState contains undeclared fields: ${unsupportedState.join(", ")}`);
        }
        const state: Record<string, unknown> = {};
        for (const [name, definition] of Object.entries(editableProps)) {
          const oldValue = current.state[name];
          if (name in current.state) {
            if (validValue(definition.type, oldValue)) {
              state[name] = oldValue;
            } else if (current.schemaRegistered) {
              state[name] = definition.default;
            } else {
              throw new Error(`initialState.${name} must be a ${definition.type}`);
            }
          } else {
            state[name] = definition.default;
          }
        }
        if (JSON.stringify(current.editableProps) === JSON.stringify(editableProps)
          && JSON.stringify(current.state) === JSON.stringify(state)
          && current.schemaRegistered) {
          return json(current);
        }
        current.editableProps = editableProps;
        current.state = state;
        current.schemaRegistered = true;
        current.updatedAt = Date.now();
        await saveState();
        return json(current);
      } catch (cause) {
        return error(cause instanceof Error ? cause.message : String(cause));
      }
    }
    if (route?.resource === "state" && request.method === "PUT") {
      const current = stickers.get(route.id);
      if (!current) return error("sticker not found", 404);
      if (!current.schemaRegistered) return error("component schema is not registered yet", 409);
      try {
        const state = validateWholeState(await request.json(), current.editableProps);
        current.state = state;
        current.updatedAt = Date.now();
        await saveState();
        return json(current);
      } catch (cause) {
        return error(cause instanceof Error ? cause.message : String(cause));
      }
    }
    if (route?.resource === "sticker" && request.method === "PATCH") {
      const current = stickers.get(route.id);
      if (!current) return error("sticker not found", 404);
      try {
        const patch = await request.json();
        if (!isObject(patch)) throw new Error("body must be a JSON object");
        const unsupported = Object.keys(patch).filter((key) => !["tsx", "x", "y", "angle"].includes(key));
        if (unsupported.length) throw new Error(`PATCH cannot change: ${unsupported.join(", ")}`);
        const normalized = normalizeCreate({
          tsx: patch.tsx ?? current.tsx,
          x: patch.x ?? current.x,
          y: patch.y ?? current.y,
          angle: patch.angle ?? current.angle,
        });
        const tsxChanged = normalized.tsx !== current.tsx;
        if (tsxChanged) await compileTsx(normalized.tsx);
        const updated: Sticker = {
          ...current,
          tsx: normalized.tsx,
          x: normalized.x,
          y: normalized.y,
          angle: normalized.angle,
          sourceHash: tsxChanged ? sourceHash(normalized.tsx) : current.sourceHash,
          updatedAt: Date.now(),
        };
        stickers.set(route.id, updated);
        await saveState();
        return json(updated);
      } catch (cause) {
        return error(cause instanceof Error ? cause.message : String(cause));
      }
    }
    if (route?.resource === "sticker" && request.method === "DELETE") {
      if (!stickers.delete(route.id)) return error("sticker not found", 404);
      await saveState();
      return new Response(null, { status: 204 });
    }
    if (url.pathname === "/api/stickers" && request.method === "POST") {
      try {
        const body = await request.json();
        if (isObject(body)) {
          const unsupported = Object.keys(body).filter((key) => !["tsx", "x", "y", "angle", "initialState"].includes(key));
          if (unsupported.length) throw new Error(`POST cannot set: ${unsupported.join(", ")}; use initialState or PUT the complete state after schema registration`);
        }
        const normalized = normalizeCreate(body);
        await compileTsx(normalized.tsx);
        const now = Date.now();
        const sticker: Sticker = {
          tsx: normalized.tsx,
          x: normalized.x,
          y: normalized.y,
          angle: normalized.angle,
          id: nextId++,
          sourceHash: sourceHash(normalized.tsx),
          editableProps: {},
          state: normalized.initialState,
          schemaRegistered: false,
          createdAt: now,
          updatedAt: now,
        };
        stickers.set(sticker.id, sticker);
        await saveState();
        return json(sticker, 201);
      } catch (cause) {
        return error(cause instanceof Error ? cause.message : String(cause));
      }
    }
    return error("not found", 404);
  },
});

await loadState();
console.log(`REST API and Chromium scene: http://127.0.0.1:${server.port}`);
console.log(`MPEG-TS output: udp://${UDP_HOST}:${UDP_PORT}`);

async function startCapture() {
  const browserInstance = await chromium.launch({
    headless: true,
    executablePath: CHROME_PATH,
    args: [
      "--disable-background-timer-throttling",
      "--disable-renderer-backgrounding",
      ...CHROMIUM_ARGS,
    ],
  });
  browser = browserInstance;
  const context = await browser.newContext({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  capturePage = page;
  await page.goto(`http://127.0.0.1:${server.port}/`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__SPIKE_READY__ === true);

  const encoder = Bun.spawn([
    "ffmpeg", "-hide_banner", "-loglevel", "warning", "-nostdin",
    "-f", "image2pipe", "-vcodec", "mjpeg", "-framerate", String(FPS), "-i", "pipe:0",
    "-re", "-f", "lavfi", "-i", "anullsrc=channel_layout=stereo:sample_rate=48000",
    "-map", "0:v:0", "-map", "1:a:0",
    "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency",
    "-r", String(FPS), "-pix_fmt", "yuv420p", "-g", String(FPS), "-keyint_min", String(FPS),
    "-bf", "0", "-sc_threshold", "0", "-x264-params", "repeat-headers=1",
    "-c:a", "aac", "-b:a", "96k", "-ar", "48000", "-ac", "2",
    "-shortest", "-mpegts_flags", "+resend_headers+initial_discontinuity",
    "-muxdelay", "0", "-muxpreload", "0", "-flush_packets", "1",
    "-f", "mpegts", `udp://${UDP_HOST}:${UDP_PORT}?pkt_size=1316`,
  ], { stdin: "pipe", stdout: "ignore", stderr: "inherit" });
  ffmpeg = encoder;

  void captureFrames(page, encoder);
}

async function captureFrames(page: Page, encoder: ReturnType<typeof Bun.spawn>) {
  while (!stopping) {
    const started = performance.now();
    try {
      const frame = await page.screenshot({ type: "jpeg", quality: 88, animations: "allow" });
      await encoder.stdin.write(frame);
    } catch (cause) {
      if (!stopping) console.error("Capture loop failed:", cause);
      break;
    }
    await Bun.sleep(Math.max(0, FRAME_INTERVAL - (performance.now() - started)));
  }
}

if (Bun.env.CAPTURE !== "0") {
  try {
    await startCapture();
  } catch (cause) {
    console.error("Could not start Chromium/FFmpeg capture:", cause);
    process.exitCode = 1;
  }
}

async function shutdown() {
  if (stopping) return;
  stopping = true;
  try { await capturePage?.close(); } catch {}
  try { await browser?.close(); } catch {}
  try { ffmpeg?.stdin.end(); } catch {}
  if (ffmpeg) {
    await Promise.race([ffmpeg.exited, Bun.sleep(1500)]);
    if (ffmpeg.exitCode === null) ffmpeg.kill();
  }
  server.stop(true);
}

process.on("SIGINT", () => void shutdown());
process.on("SIGTERM", () => void shutdown());
