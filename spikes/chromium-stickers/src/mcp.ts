import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const API = (Bun.env.BRONZECAST_API_URL ?? "http://127.0.0.1:8091").replace(/\/$/, "");
const STATE = z.record(z.string(), z.unknown());

async function request(path: string, method = "GET", body?: unknown) {
  const response = await fetch(`${API}${path}`, {
    method,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let result: unknown;
  try {
    result = text ? JSON.parse(text) : null;
  } catch {
    result = text;
  }
  if (!response.ok) {
    const message = typeof result === "object" && result !== null && "error" in result
      ? String((result as { error: unknown }).error)
      : `${response.status} ${response.statusText}: ${text}`;
    throw new Error(message);
  }
  return result;
}

function stickerSummary(sticker: Record<string, unknown>) {
  return {
    id: sticker.id,
    x: sticker.x,
    y: sticker.y,
    angle: sticker.angle,
    schemaRegistered: sticker.schemaRegistered,
    editableProps: sticker.editableProps,
    state: sticker.state,
    createdAt: sticker.createdAt,
    updatedAt: sticker.updatedAt,
  };
}

function result(value: unknown) {
  return { content: [{ type: "text" as const, text: JSON.stringify(value, null, 2) }] };
}

const server = new McpServer({
  name: "bronzecast-tsx-stickers",
  version: "0.1.0",
});

server.registerTool("list_stickers", {
  title: "List TSX stickers",
  description: "List component source metadata, editable state schemas, current state snapshots, and placement for every sticker.",
  inputSchema: {},
  annotations: { readOnlyHint: true, idempotentHint: true },
}, async () => {
  const stickers = await request("/api/stickers") as Array<Record<string, unknown>>;
  return result(stickers.map(stickerSummary));
});

server.registerTool("get_sticker", {
  title: "Get TSX sticker",
  description: "Read one sticker's editable schema and current backend-owned state snapshot.",
  inputSchema: { id: z.number().int().positive() },
  annotations: { readOnlyHint: true, idempotentHint: true },
}, async ({ id }) => {
  const sticker = await request(`/api/stickers/${id}`) as Record<string, unknown>;
  return result(stickerSummary(sticker));
});

server.registerTool("create_sticker", {
  title: "Create TSX sticker",
  description: "Create a self-contained React TSX sticker. Export editableProps from the component module; the rendered app registers its schema and supplies state. initialState, when provided, is the whole initial state object.",
  inputSchema: {
    tsx: z.string().min(1).max(64 * 1024),
    x: z.number().min(0).max(960),
    y: z.number().min(0).max(540),
    angle: z.number().optional(),
    initialState: STATE.optional(),
  },
  annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false },
}, async ({ tsx, x, y, angle, initialState }) => {
  const created = await request("/api/stickers", "POST", {
    tsx,
    x,
    y,
    angle: angle ?? 0,
    ...(initialState === undefined ? {} : { initialState }),
  }) as Record<string, unknown>;
  const id = created.id as number;

  // The running Chromium page imports the new component and registers its schema.
  // Wait briefly so the caller can immediately inspect/write the state if ready.
  const until = Date.now() + 3000;
  let current = created;
  while (Date.now() < until && current.schemaRegistered !== true) {
    await Bun.sleep(100);
    try {
      current = await request(`/api/stickers/${id}`) as Record<string, unknown>;
    } catch {
      break;
    }
  }
  return result({ ...stickerSummary(current), schemaPending: current.schemaRegistered !== true });
});

server.registerTool("update_sticker", {
  title: "Update TSX sticker",
  description: "Replace a sticker's TSX source or change its position/angle. Use set_sticker_state to replace its editable values.",
  inputSchema: {
    id: z.number().int().positive(),
    tsx: z.string().min(1).max(64 * 1024).optional(),
    x: z.number().min(0).max(960).optional(),
    y: z.number().min(0).max(540).optional(),
    angle: z.number().optional(),
  },
  annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true },
}, async ({ id, ...patch }) => {
  if (Object.values(patch).every((value) => value === undefined)) {
    throw new Error("provide at least one of tsx, x, y, or angle");
  }
  const updated = await request(`/api/stickers/${id}`, "PATCH", patch) as Record<string, unknown>;
  return result(stickerSummary(updated));
});

server.registerTool("set_sticker_state", {
  title: "Replace sticker state",
  description: "Replace the entire editable state object for a sticker. Keys and value types must match the component's editableProps schema; every field is required.",
  inputSchema: {
    id: z.number().int().positive(),
    state: STATE,
  },
  annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true },
}, async ({ id, state }) => {
  const updated = await request(`/api/stickers/${id}/state`, "PUT", state) as Record<string, unknown>;
  return result(stickerSummary(updated));
});

server.registerTool("remove_sticker", {
  title: "Remove TSX sticker",
  description: "Remove a sticker component and its persisted state.",
  inputSchema: { id: z.number().int().positive() },
  annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true },
}, async ({ id }) => {
  await request(`/api/stickers/${id}`, "DELETE");
  return result({ id, removed: true });
});

await server.connect(new StdioServerTransport());
