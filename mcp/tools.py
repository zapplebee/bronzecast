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
STICKER_KINDS = {"countdown", "timer", "todo", "memo"}


class ToolError(Exception):
    pass


TOOLS = [
    {
        "name": "bronzecast_get_status",
        "title": "Get dashboard status",
        "description": (
            "Read the active character, persistent stickers, notification queue, "
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
        "name": "bronzecast_list_notifications",
        "title": "List notifications",
        "description": "Read the notifications currently shown on the dashboard.",
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
        "name": "bronzecast_send_notification",
        "title": "Send notification",
        "description": (
            "Add a notification card to the live video. Titles are limited to 48 "
            "characters and messages to 320; only newlines are allowed as "
            "control characters."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short heading; defaults to 'Notification'.",
                    "maxLength": 48,
                },
                "message": {
                    "type": "string",
                    "description": "Body text, up to 320 characters.",
                    "maxLength": 320,
                },
            },
            "required": ["message"],
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
        "name": "bronzecast_clear_notifications",
        "title": "Clear notifications",
        "description": "Remove every notification from the live video dashboard.",
        "inputSchema": {
            "type": "object",
            "properties": {},
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
        "name": "bronzecast_list_stickers",
        "title": "List stickers",
        "description": (
            "Read every persistent sticker on the video board, including "
            "countdowns, tomato timers, todos, and memos."
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
            "Create an agent-controlled sticker at the supplied board x/y; there "
            "are no seeded stickers or placement defaults. angle is optional "
            "clockwise degrees. Use todo for completable tasks and memo for "
            "information to keep visible. Countdowns need a YYYY-MM-DD target; "
            "timers require an explicit duration_seconds."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["countdown", "timer", "todo", "memo"],
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
                    "description": "Optional memo title; memo requires title or text.",
                },
                "text": {
                    "type": "string",
                    "maxLength": 160,
                    "description": "Text for countdown, todo, or memo; countdown/todo require it.",
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
            },
            "required": ["kind", "x", "y"],
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
            "Move, rotate, or edit a sticker. x and y are unrotated top-left "
            "board coordinates; angle is clockwise degrees. Timers accept "
            "action (start, pause, reset, set); countdowns accept target; "
            "todos accept done; memos accept title and text."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Sticker id from bronzecast_list_stickers.",
                },
                "x": {"type": "number", "minimum": 0, "maximum": 960},
                "y": {"type": "number", "minimum": 0, "maximum": 540},
                "angle": {"type": "number", "description": "Clockwise degrees around sticker center."},
                "title": {"type": "string", "maxLength": 48},
                "text": {"type": "string", "maxLength": 160},
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
    "bronzecast_send_notification",
    "bronzecast_clear_notifications",
    "bronzecast_add_sticker",
    "bronzecast_update_sticker",
    "bronzecast_remove_sticker",
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


def _validate_notification(arguments):
    title = arguments.get("title", "Notification")
    message = arguments.get("message")
    if not isinstance(title, str) or not isinstance(message, str):
        raise ToolError("title and message must be strings")
    title = title.strip()
    message = message.strip()
    if not title:
        raise ToolError("title must not be empty")
    if len(title) > 48:
        raise ToolError("title must be 48 characters or fewer")
    if not message:
        raise ToolError("message must not be empty")
    if len(message) > 320:
        raise ToolError("message must be 320 characters or fewer")
    if any(_control(char) for char in title):
        raise ToolError("title must not contain control characters")
    if any(char != "\n" and _control(char) for char in message):
        raise ToolError("message only allows newlines as control characters")
    return {"title": title, "message": message}


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
        "kind", "x", "y", "angle", "title", "text", "target",
        "duration_seconds", "done",
    }
    if unknown:
        raise ToolError(f"unknown sticker field: {sorted(unknown)[0]}")
    kind = arguments.get("kind")
    if kind not in STICKER_KINDS:
        raise ToolError("kind must be countdown, timer, todo, or memo")
    payload = {"kind": kind}
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
    if "text" in arguments or kind in {"countdown", "todo"}:
        payload["text"] = _sticker_text(
            arguments.get("text"),
            required=kind in {"countdown", "todo"},
            newlines=kind in {"todo", "memo"},
        )
    target = arguments.get("target")
    duration = arguments.get("duration_seconds")
    done = arguments.get("done")
    if target is not None and kind != "countdown":
        raise ToolError("target only applies to countdown stickers")
    if duration is not None and kind != "timer":
        raise ToolError("duration_seconds only applies to timer stickers")
    if done is not None and kind != "todo":
        raise ToolError("done only applies to todo stickers")
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
    return payload


def _validate_sticker_update(arguments):
    unknown = set(arguments) - {
        "id", "x", "y", "angle", "title", "text", "target", "done",
        "action", "duration_seconds",
    }
    if unknown:
        raise ToolError(f"unknown sticker field: {sorted(unknown)[0]}")
    payload = {}
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
            arguments["text"], required=False, newlines=True
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
    character, notifications, stickers, stream = await asyncio.gather(
        _json("GET", settings.generator_url + "/api/character"),
        _json("GET", settings.generator_url + "/api/notifications"),
        _json("GET", settings.generator_url + "/api/stickers"),
        _json("GET", settings.playout_url + "/health"),
        return_exceptions=True,
    )
    payload = {}
    for key, value in (
        ("character", character),
        ("notifications", notifications),
        ("stickers", stickers),
        ("stream", stream),
    ):
        payload[key] = {"error": str(value)} if isinstance(value, Exception) else value
    core_failed = isinstance(character, Exception) and isinstance(notifications, Exception)
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

    if name == "bronzecast_list_notifications":
        return {"content": [{"type": "text", "text": _text(await _json("GET", settings.generator_url + "/api/notifications"))}]}

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

    if name == "bronzecast_send_notification":
        result = await _json(
            "POST",
            settings.generator_url + "/api/notifications",
            _validate_notification(arguments),
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

    if name == "bronzecast_clear_notifications":
        await _json("DELETE", settings.generator_url + "/api/notifications")
        return {"content": [{"type": "text", "text": "All notifications cleared."}]}

    raise ToolError(f"Unknown tool: {name}")
