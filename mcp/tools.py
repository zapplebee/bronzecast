"""MCP tool definitions and dispatch onto the renderer/playout HTTP APIs."""
import asyncio
import base64
import datetime
import json
import math
import re

import httpx

TIMEOUT = 5.0
CHARACTER_ID_RE = re.compile(r"^[^\s\x00-\x1f\x7f]{1,64}$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
STICKER_KINDS = {"countdown", "timer", "todo", "memo", "notification", "clock", "art"}
STICKER_APPEARANCES = (
    "jack_o_lantern",
    "ghost",
    "black_cat",
    "bat",
    "candy_pail",
    "turkey",
    "cornucopia",
    "maple_leaf",
    "pumpkin_pie",
    "christmas_tree",
    "wrapped_gift",
    "snowman",
    "reindeer",
    "gingerbread_person",
    "candy_cane",
    "fireworks",
    "lunar_new_year_lantern",
    "diya",
    "heart",
    "easter_bunny",
    "decorated_egg",
    "tulip",
    "sun",
    "ice_cream_cone",
    "custom_svg",
)
STICKER_APPEARANCE_SET = set(STICKER_APPEARANCES)


class ToolError(Exception):
    pass


TOOLS = [
    {
        "name": "bronzecast_get_status",
        "title": "Get dashboard status",
        "description": (
            "Read the active character, all stickers including expiring notifications, "
            "and live playout encoder state in one call."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_get_snapshot",
        "title": "Get rendered board snapshot",
        "description": (
            "Return the latest complete 960x540 renderer frame as a PNG image. "
            "Use it with bronzecast_list_stickers to inspect placement, then "
            "adjust sticker x, y, or angle and request another snapshot."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_list_characters",
        "title": "List characters",
        "description": (
            "List the characters available on the dashboard and which one is "
            "active or requested."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_switch_character",
        "title": "Switch character",
        "description": (
            "Request a different character for the live stream. The change is "
            "accepted immediately and appears on the next rendered frame."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Character id from bronzecast_list_characters.",
                }
            },
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_list_stickers",
        "title": "List stickers",
        "description": (
            "Read every sticker, including appearance, behavior, and returned "
            "expiry metadata for expiring notification stickers."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_add_sticker",
        "title": "Add sticker",
        "description": (
            "Create an agent-controlled sticker by choosing an appearance and "
            "kind (timer, countdown, todo, memo, notification, clock, or art). "
            "Supply x and y; there are no seeded stickers or placement defaults. "
            "custom_svg requires inline vector SVG. Existing behavior kinds render "
            "their usual content over that artwork; art is SVG-only. Notifications "
            "require an expiry TTL; clocks display local time and date."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["countdown", "timer", "todo", "memo", "notification", "clock", "art"],
                },
                "appearance": {
                    "type": "string",
                    "enum": list(STICKER_APPEARANCES),
                    "description": "One of 24 preset motifs, or custom_svg when supplying SVG artwork.",
                },
                "x": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 960,
                    "description": "Required unrotated top-left x in board pixels.",
                },
                "y": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 540,
                    "description": "Required unrotated top-left y in board pixels.",
                },
                "angle": {
                    "type": "number",
                    "description": "Clockwise degrees around sticker center; defaults to 0.",
                },
                "title": {
                    "type": "string",
                    "maxLength": 48,
                    "description": "Optional memo or notification title; notification defaults to 'Notification'.",
                },
                "text": {
                    "type": "string",
                    "maxLength": 320,
                    "description": "Text for countdown, todo, memo, or notification; countdown/todo/notification require it.",
                },
                "target": {
                    "type": "string",
                    "description": "YYYY-MM-DD date for a countdown.",
                },
                "duration_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "description": "Required for timers; duration in seconds (1–3600).",
                },
                "done": {
                    "type": "boolean",
                    "description": "Initial todo completion state.",
                },
                "expires_in_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 604800,
                    "description": "Required for notification stickers; expiry is hidden on the artwork.",
                },
                "svg": {
                    "type": "string",
                    "maxLength": 32768,
                    "description": "Required with custom_svg; bounded vector SVG artwork, without text or embedded images.",
                },
            },
            "required": ["kind", "appearance", "x", "y"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": False,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_update_sticker",
        "title": "Update sticker",
        "description": (
            "Change a sticker's appearance, move it, rotate it, or edit its "
            "behavior. x/y are unrotated top-left board coordinates; angle is "
            "clockwise degrees. Timers accept "
            "action (start, pause, reset, set); countdowns accept target; "
            "todos accept done; memos and notifications accept title and text. "
            "Clock and notification expiry are fixed at creation. SVG artwork can "
            "be replaced on custom_svg stickers; art stickers have no text overlay."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Sticker id from bronzecast_list_stickers.",
                },
                "appearance": {
                    "type": "string",
                    "enum": list(STICKER_APPEARANCES),
                },
                "x": {"type": "number", "minimum": 0, "maximum": 960},
                "y": {"type": "number", "minimum": 0, "maximum": 540},
                "angle": {"type": "number", "description": "Clockwise degrees around sticker center."},
                "title": {"type": "string", "maxLength": 48},
                "text": {"type": "string", "maxLength": 320},
                "target": {
                    "type": "string",
                    "description": "New YYYY-MM-DD date for a countdown.",
                },
                "done": {"type": "boolean"},
                "action": {
                    "type": "string",
                    "enum": ["start", "pause", "reset", "set"],
                },
                "duration_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                },
                "svg": {
                    "type": "string",
                    "maxLength": 32768,
                    "description": "Replacement vector SVG for a custom_svg sticker.",
                },
            },
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_remove_sticker",
        "title": "Remove sticker",
        "description": "Delete a sticker from the video board.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Sticker id from bronzecast_list_stickers.",
                }
            },
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": True,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_list_stickers",
        "title": "List TSX stickers",
        "description": (
            "List custom React/TSX stickers with their editable state schema, "
            "current shared state, and placement."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_get_sticker",
        "title": "Get TSX sticker state",
        "description": "Read a TSX sticker's editableProps schema and current backend-owned state.",
        "inputSchema": {
            "type": "object",
            "properties": {"id": {"type": "integer", "minimum": 1}},
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_create_sticker",
        "title": "Create TSX sticker",
        "description": (
            "Create a custom React/TSX sticker. Export editableProps from the "
            "component module. The app registers the schema and passes the current "
            "shared snapshot as the component's state prop. Time-driven components "
            "may import useAnimationFrame from @bronzecast/sticker-runtime. "
            "initial_state, if supplied, is the complete initial state object."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "tsx": {"type": "string", "maxLength": 65536},
                "x": {"type": "number", "minimum": 0, "maximum": 960},
                "y": {"type": "number", "minimum": 0, "maximum": 540},
                "angle": {"type": "number"},
                "initial_state": {"type": "object", "additionalProperties": True},
            },
            "required": ["tsx", "x", "y"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": False,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_update_sticker",
        "title": "Update TSX sticker",
        "description": "Replace a TSX component or change its placement. Use bronzecast_tsx_set_sticker_state to change editable values.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "integer", "minimum": 1},
                "tsx": {"type": "string", "maxLength": 65536},
                "x": {"type": "number", "minimum": 0, "maximum": 960},
                "y": {"type": "number", "minimum": 0, "maximum": 540},
                "angle": {"type": "number"},
            },
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_set_sticker_state",
        "title": "Replace TSX sticker state",
        "description": "Replace the complete editable state object. Keys and value types must match editableProps; state is shared with every renderer/client.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "integer", "minimum": 1},
                "state": {"type": "object", "additionalProperties": True},
            },
            "required": ["id", "state"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_tsx_remove_sticker",
        "title": "Remove TSX sticker",
        "description": "Remove a custom TSX sticker and its persisted shared state.",
        "inputSchema": {
            "type": "object",
            "properties": {"id": {"type": "integer", "minimum": 1}},
            "required": ["id"],
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": False,
            "destructiveHint": True,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
    {
        "name": "bronzecast_stream_status",
        "title": "Stream status",
        "description": (
            "Report the playout encoder state, including whether the renderer "
            "feed is live or the standby card is on air."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": False,
        },
        "annotations": {
            "readOnlyHint": True,
            "destructiveHint": False,
            "idempotentHint": True,
            "openWorldHint": False,
        },
    },
]

WRITE_TOOLS = {
    "bronzecast_switch_character",
    "bronzecast_add_sticker",
    "bronzecast_update_sticker",
    "bronzecast_remove_sticker",
    "bronzecast_tsx_create_sticker",
    "bronzecast_tsx_update_sticker",
    "bronzecast_tsx_set_sticker_state",
    "bronzecast_tsx_remove_sticker",
}


async def _json(method, url, payload=None):
    try:
        async with httpx.AsyncClient(timeout=TIMEOUT) as client:
            response = await client.request(method, url, json=payload)
    except httpx.HTTPError as error:
        raise ToolError(f"BRONZECAST service unreachable: {error}") from error
    if response.status_code >= 400:
        detail = response.text.strip()[:500]
        raise ToolError(
            f"{method} {url} returned HTTP {response.status_code}: {detail}"
        )
    if response.status_code == 204 or not response.content:
        return None
    try:
        return response.json()
    except ValueError:
        return {"text": response.text}


async def _snapshot_png(url):
    try:
        async with httpx.AsyncClient(timeout=TIMEOUT) as client:
            response = await client.get(url)
    except httpx.HTTPError as error:
        raise ToolError(f"BRONZECAST snapshot unavailable: {error}") from error
    if response.status_code >= 400:
        detail = response.text.strip()[:500]
        raise ToolError(
            f"GET {url} returned HTTP {response.status_code}: {detail}"
        )
    content_type = response.headers.get("content-type", "").split(";", 1)[0].strip()
    if content_type != "image/png":
        raise ToolError(f"snapshot returned unexpected content type: {content_type or 'missing'}")
    if not response.content:
        raise ToolError("snapshot response was empty")
    return response.content


def _control(char):
    return ord(char) < 32 or ord(char) == 127


def _sticker_id(arguments):
    value = arguments.get("id")
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ToolError("id must be a positive integer")
    return value


def _coordinate(arguments, name, maximum):
    value = arguments.get(name)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ToolError(f"{name} must be a number")
    if not math.isfinite(value) or not 0 <= value <= maximum:
        raise ToolError(f"{name} must be between 0 and {maximum}")
    return value


def _angle(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ToolError("angle must be a number")
    if not math.isfinite(value):
        raise ToolError("angle must be finite")
    return value


def _tsx_source(value):
    if not isinstance(value, str) or not value.strip():
        raise ToolError("tsx must be a non-empty component source string")
    if len(value.encode("utf-8")) > 65536:
        raise ToolError("tsx must be 64 KiB or smaller")
    if "export default" not in value:
        raise ToolError("tsx must default-export its React component")
    return value


def _tsx_state(value, name="state"):
    if not isinstance(value, dict):
        raise ToolError(f"{name} must be a JSON object containing the complete state")
    try:
        json.dumps(value, allow_nan=False)
    except (TypeError, ValueError) as error:
        raise ToolError(f"{name} must contain only JSON values: {error}") from error
    return value


def _tsx_sticker_summary(sticker):
    return {
        key: sticker[key]
        for key in (
            "id", "x", "y", "angle", "schemaRegistered", "editableProps",
            "state", "createdAt", "updatedAt",
        )
        if key in sticker
    }


async def _wait_for_tsx_schema(settings, sticker_id, timeout=5.0):
    deadline = asyncio.get_running_loop().time() + timeout
    latest = None
    while asyncio.get_running_loop().time() < deadline:
        latest = await _json(
            "GET", settings.tsx_api_url + f"/api/stickers/{sticker_id}"
        )
        if latest.get("schemaRegistered") is True:
            return latest
        await asyncio.sleep(0.1)
    return latest


def _sticker_text(value, name="text", maximum=160, required=True, newlines=False):
    if value is None:
        value = ""
    if not isinstance(value, str):
        raise ToolError(f"{name} must be a string")
    value = value.strip()
    if required and not value:
        raise ToolError(f"{name} must not be empty")
    if len(value) > maximum:
        raise ToolError(f"{name} must be {maximum} characters or fewer")
    if any(_control(char) and not (newlines and char == "\n") for char in value):
        raise ToolError(
            f"{name} only allows newlines as control characters"
            if newlines
            else f"{name} must not contain control characters"
        )
    return value


def _sticker_duration(value):
    if isinstance(value, bool) or not isinstance(value, int):
        raise ToolError("duration_seconds must be an integer")
    if not 1 <= value <= 3600:
        raise ToolError("duration_seconds must be between 1 and 3600")
    return value


def _sticker_ttl(value):
    if isinstance(value, bool) or not isinstance(value, int):
        raise ToolError("expires_in_seconds must be an integer")
    if not 1 <= value <= 604800:
        raise ToolError("expires_in_seconds must be between 1 and 604800")
    return value


def _sticker_svg(value):
    if not isinstance(value, str):
        raise ToolError("svg must be a string")
    if len(value.encode("utf-8")) > 32 * 1024:
        raise ToolError("svg must be 32768 bytes or fewer")
    if "<svg" not in value:
        raise ToolError("svg must contain an <svg> element")
    return value


def _sticker_date(value):
    if not isinstance(value, str) or not DATE_RE.fullmatch(value):
        raise ToolError("target must be a valid YYYY-MM-DD date")
    try:
        datetime.date.fromisoformat(value)
    except ValueError as error:
        raise ToolError("target must be a valid YYYY-MM-DD date") from error
    return value


def _validate_sticker_add(arguments):
    unknown = set(arguments) - {
        "kind", "appearance", "x", "y", "angle", "title", "text",
        "target", "duration_seconds", "done", "expires_in_seconds", "svg",
    }
    if unknown:
        raise ToolError(f"unknown sticker field: {sorted(unknown)[0]}")
    kind = arguments.get("kind")
    if kind not in STICKER_KINDS:
        raise ToolError("kind must be countdown, timer, todo, memo, notification, clock, or art")
    appearance = arguments.get("appearance")
    if not isinstance(appearance, str) or appearance not in STICKER_APPEARANCE_SET:
        raise ToolError("appearance must be selected from the available sticker appearance list")
    payload = {"kind": kind, "appearance": appearance}
    svg = None
    if "svg" in arguments:
        svg = _sticker_svg(arguments["svg"])
        if appearance != "custom_svg":
            raise ToolError("svg requires appearance 'custom_svg'")
        payload["svg"] = svg
    if appearance == "custom_svg" and svg is None:
        raise ToolError("appearance 'custom_svg' requires svg artwork")
    for name, maximum in (("x", 960), ("y", 540)):
        if name not in arguments:
            raise ToolError(f"{name} is required")
        payload[name] = _coordinate(arguments, name, maximum)
    if "angle" in arguments:
        payload["angle"] = _angle(arguments["angle"])
    if "title" in arguments:
        payload["title"] = _sticker_text(
            arguments["title"], name="title", maximum=48, required=False
        )
    if "text" in arguments or kind in {"countdown", "todo", "notification"}:
        payload["text"] = _sticker_text(
            arguments.get("text"),
            maximum=320 if kind == "notification" else 160,
            required=kind in {"countdown", "todo", "notification"},
            newlines=kind in {"todo", "memo", "notification"},
        )
    target = arguments.get("target")
    duration = arguments.get("duration_seconds")
    done = arguments.get("done")
    ttl = arguments.get("expires_in_seconds")
    if target is not None and kind != "countdown":
        raise ToolError("target only applies to countdown stickers")
    if duration is not None and kind != "timer":
        raise ToolError("duration_seconds only applies to timer stickers")
    if done is not None and kind != "todo":
        raise ToolError("done only applies to todo stickers")
    if "expires_in_seconds" in arguments and kind != "notification":
        raise ToolError("expires_in_seconds only applies to notification stickers")
    if done is not None and not isinstance(done, bool):
        raise ToolError("done must be a boolean")
    if kind == "timer" and any(
        name in arguments for name in ("title", "text", "target", "done")
    ):
        raise ToolError("timer accepts only duration_seconds in addition to position and angle")
    if kind == "countdown" and any(
        name in arguments for name in ("title", "duration_seconds", "done")
    ):
        raise ToolError("countdown does not accept title, duration_seconds, or done")
    if kind == "todo" and any(
        name in arguments for name in ("title", "target", "duration_seconds")
    ):
        raise ToolError("todo does not accept title, target, or duration_seconds")
    if kind == "memo" and any(
        name in arguments for name in ("target", "duration_seconds", "done")
    ):
        raise ToolError("memo does not accept target, duration_seconds, or done")
    if kind == "notification" and any(
        name in arguments for name in ("target", "duration_seconds", "done")
    ):
        raise ToolError("notification does not accept target, duration_seconds, or done")
    if kind == "clock" and any(
        name in arguments
        for name in (
            "title", "text", "target", "duration_seconds", "done", "expires_in_seconds"
        )
    ):
        raise ToolError("clock accepts only appearance, position, and angle")
    if kind == "art":
        if appearance != "custom_svg":
            raise ToolError("art requires appearance 'custom_svg'")
        if any(
            name in arguments
            for name in (
                "title", "text", "target", "duration_seconds", "done",
                "expires_in_seconds",
            )
        ):
            raise ToolError("art stickers contain only SVG artwork, position, and angle")
    if kind == "countdown":
        if target is None:
            raise ToolError("countdown requires target (YYYY-MM-DD)")
        payload["target"] = _sticker_date(target)
    elif kind == "timer" and duration is not None:
        payload["duration_seconds"] = _sticker_duration(duration)
    elif kind == "timer":
        raise ToolError("timer requires duration_seconds")
    elif kind == "todo":
        if done is not None:
            payload["done"] = done
    elif kind == "memo":
        if not payload.get("title", "").strip() and not payload.get("text", "").strip():
            raise ToolError("memo requires a non-empty title or text")
    elif kind == "notification":
        if "title" in payload and not payload["title"]:
            raise ToolError("notification title must not be empty")
        if ttl is None:
            raise ToolError("notification requires expires_in_seconds")
        payload["expires_in_seconds"] = _sticker_ttl(ttl)
    return payload


def _validate_sticker_update(arguments):
    unknown = set(arguments) - {
        "id", "appearance", "x", "y", "angle", "title", "text",
        "target", "done", "action", "duration_seconds", "svg",
    }
    if unknown:
        raise ToolError(f"unknown sticker field: {sorted(unknown)[0]}")
    payload = {}
    if "appearance" in arguments:
        if not isinstance(arguments["appearance"], str) or arguments["appearance"] not in STICKER_APPEARANCE_SET:
            raise ToolError("appearance must be selected from the available sticker appearance list")
        payload["appearance"] = arguments["appearance"]
    if "svg" in arguments:
        payload["svg"] = _sticker_svg(arguments["svg"])
        if "appearance" in arguments and arguments["appearance"] != "custom_svg":
            raise ToolError("svg requires appearance 'custom_svg'")
    for name, maximum in (("x", 960), ("y", 540)):
        if name in arguments:
            payload[name] = _coordinate(arguments, name, maximum)
    if "angle" in arguments:
        payload["angle"] = _angle(arguments["angle"])
    if "title" in arguments:
        payload["title"] = _sticker_text(
            arguments["title"], name="title", maximum=48, required=False
        )
    if "text" in arguments:
        payload["text"] = _sticker_text(
            arguments["text"], maximum=320, required=False, newlines=True
        )
    if "target" in arguments:
        payload["target"] = _sticker_date(arguments["target"])
    if "done" in arguments:
        if not isinstance(arguments["done"], bool):
            raise ToolError("done must be a boolean")
        payload["done"] = arguments["done"]
    if "action" in arguments:
        if arguments["action"] not in {"start", "pause", "reset", "set"}:
            raise ToolError("action must be start, pause, reset, or set")
        payload["action"] = arguments["action"]
    if "duration_seconds" in arguments:
        payload["duration_seconds"] = _sticker_duration(
            arguments["duration_seconds"]
        )
    if "duration_seconds" in payload and payload.get("action") != "set":
        raise ToolError("duration_seconds is writable only with action 'set'")
    if payload.get("action") == "set" and "duration_seconds" not in payload:
        raise ToolError("action 'set' requires duration_seconds")
    if not payload:
        raise ToolError("at least one sticker field must be provided")
    return payload


def _text(value):
    return json.dumps(value, indent=2, sort_keys=True)


async def _status_payload(settings):
    character, stickers, stream = await asyncio.gather(
        _json("GET", settings.generator_url + "/api/character"),
        _json("GET", settings.generator_url + "/api/stickers"),
        _json("GET", settings.playout_url + "/health"),
        return_exceptions=True,
    )
    payload = {}
    for key, value in (
        ("character", character),
        ("stickers", stickers),
        ("stream", stream),
    ):
        payload[key] = {"error": str(value)} if isinstance(value, Exception) else value
    core_failed = isinstance(character, Exception) and isinstance(stickers, Exception)
    return payload, core_failed


async def call_tool(name, arguments, settings):
    if name == "bronzecast_get_status":
        payload, failed = await _status_payload(settings)
        return {"content": [{"type": "text", "text": _text(payload)}], "isError": failed}

    if name == "bronzecast_get_snapshot":
        if arguments:
            raise ToolError("bronzecast_get_snapshot does not accept arguments")
        png = await _snapshot_png(settings.generator_url + "/api/snapshot")
        return {
            "content": [
                {
                    "type": "image",
                    "data": base64.b64encode(png).decode("ascii"),
                    "mimeType": "image/png",
                }
            ]
        }

    if name == "bronzecast_list_characters":
        return {"content": [{"type": "text", "text": _text(await _json("GET", settings.generator_url + "/api/character"))}]}

    if name == "bronzecast_tsx_list_stickers":
        stickers = await _json("GET", settings.tsx_api_url + "/api/stickers")
        return {"content": [{"type": "text", "text": _text([_tsx_sticker_summary(item) for item in stickers])}]}

    if name == "bronzecast_tsx_get_sticker":
        sticker_id = _sticker_id(arguments)
        sticker = await _json("GET", settings.tsx_api_url + f"/api/stickers/{sticker_id}")
        return {"content": [{"type": "text", "text": _text(_tsx_sticker_summary(sticker))}]}

    if name == "bronzecast_tsx_create_sticker":
        tsx = _tsx_source(arguments.get("tsx"))
        x = _coordinate(arguments, "x", 960)
        y = _coordinate(arguments, "y", 540)
        angle = _angle(arguments.get("angle", 0))
        payload = {"tsx": tsx, "x": x, "y": y, "angle": angle}
        if "initial_state" in arguments:
            payload["initialState"] = _tsx_state(arguments["initial_state"], "initial_state")
        sticker = await _json("POST", settings.tsx_api_url + "/api/stickers", payload)
        sticker_id = sticker["id"]
        sticker = await _wait_for_tsx_schema(settings, sticker_id)
        summary = _tsx_sticker_summary(sticker)
        summary["schemaPending"] = sticker.get("schemaRegistered") is not True
        return {"content": [{"type": "text", "text": _text(summary)}]}

    if name == "bronzecast_tsx_update_sticker":
        sticker_id = _sticker_id(arguments)
        payload = {}
        if "tsx" in arguments:
            payload["tsx"] = _tsx_source(arguments["tsx"])
        for field, maximum in (("x", 960), ("y", 540)):
            if field in arguments:
                payload[field] = _coordinate(arguments, field, maximum)
        if "angle" in arguments:
            payload["angle"] = _angle(arguments["angle"])
        if not payload:
            raise ToolError("provide at least one of tsx, x, y, or angle")
        sticker = await _json(
            "PATCH", settings.tsx_api_url + f"/api/stickers/{sticker_id}", payload
        )
        if "tsx" in payload:
            sticker = await _wait_for_tsx_schema(settings, sticker_id)
        return {"content": [{"type": "text", "text": _text(_tsx_sticker_summary(sticker))}]}

    if name == "bronzecast_tsx_set_sticker_state":
        sticker_id = _sticker_id(arguments)
        state = _tsx_state(arguments.get("state"))
        sticker = await _json(
            "PUT",
            settings.tsx_api_url + f"/api/stickers/{sticker_id}/state",
            state,
        )
        return {"content": [{"type": "text", "text": _text(_tsx_sticker_summary(sticker))}]}

    if name == "bronzecast_tsx_remove_sticker":
        sticker_id = _sticker_id(arguments)
        await _json("DELETE", settings.tsx_api_url + f"/api/stickers/{sticker_id}")
        return {"content": [{"type": "text", "text": f"TSX sticker {sticker_id} removed."}]}

    if name == "bronzecast_list_stickers":
        return {
            "content": [
                {
                    "type": "text",
                    "text": _text(
                        await _json("GET", settings.generator_url + "/api/stickers")
                    ),
                }
            ]
        }

    if name == "bronzecast_add_sticker":
        result = await _json(
            "POST",
            settings.generator_url + "/api/stickers",
            _validate_sticker_add(arguments),
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

    if name == "bronzecast_update_sticker":
        sticker_id = _sticker_id(arguments)
        result = await _json(
            "PATCH",
            settings.generator_url + f"/api/stickers/{sticker_id}",
            _validate_sticker_update(arguments),
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

    if name == "bronzecast_remove_sticker":
        sticker_id = _sticker_id(arguments)
        await _json(
            "DELETE",
            settings.generator_url + f"/api/stickers/{sticker_id}",
        )
        return {"content": [{"type": "text", "text": f"Sticker {sticker_id} removed."}]}

    if name == "bronzecast_stream_status":
        return {"content": [{"type": "text", "text": _text(await _json("GET", settings.playout_url + "/health"))}]}

    if name == "bronzecast_switch_character":
        character_id = arguments.get("id")
        if not isinstance(character_id, str) or not CHARACTER_ID_RE.fullmatch(character_id):
            raise ToolError("id must be a non-empty character id without whitespace")
        result = await _json(
            "POST",
            settings.generator_url + "/api/character",
            {"id": character_id},
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

    raise ToolError(f"Unknown tool: {name}")
