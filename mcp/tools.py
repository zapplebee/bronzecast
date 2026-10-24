"""MCP tool definitions and dispatch onto the renderer/playout HTTP APIs."""
import asyncio
import datetime
import json
import math
import re

import httpx

TIMEOUT = 5.0
CHARACTER_ID_RE = re.compile(r"^[^\s\x00-\x1f\x7f]{1,64}$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
STICKER_KINDS = {"countdown", "timer", "reminder", "todo"}


class ToolError(Exception):
    pass


TOOLS = [
    {
        "name": "bronzecast_get_status",
        "title": "Get dashboard status",
        "description": (
            "Read the active character, persistent open item, stickers, "
            "notification queue, and live playout encoder state in one call."
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
        "name": "bronzecast_get_open_item",
        "title": "Get open item",
        "description": (
            "Read the persistent reminder pinned to the dashboard. Returns null "
            "when nothing is open."
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
        "name": "bronzecast_set_open_item",
        "title": "Pin open item",
        "description": (
            "Pin a persistent reminder card to the live video, like a string on "
            "a finger. Unlike notifications it stays until cleared and survives "
            "renderer restarts. The title appears at the bottom of the tag; the "
            "optional note fills the body. Titles are limited to 48 characters "
            "and notes to 160; only note newlines are allowed as control "
            "characters."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "The thing to remember; appears at the bottom.",
                    "maxLength": 48,
                },
                "note": {
                    "type": "string",
                    "description": "Optional detail, up to 160 characters.",
                    "maxLength": 160,
                },
            },
            "required": ["title"],
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
        "name": "bronzecast_clear_open_item",
        "title": "Clear open item",
        "description": "Remove the persistent open item from the dashboard.",
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
        "name": "bronzecast_get_timer",
        "title": "Get tomato timer",
        "description": (
            "Read the tomato focus timer's duration, remaining seconds, and "
            "state (ready, running, paused, or done)."
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
        "name": "bronzecast_set_timer",
        "title": "Set tomato timer",
        "description": (
            "Start, pause, reset, or set the tomato focus timer. "
            "duration_seconds is 1–3600 and resets the timer when provided; "
            "action 'set' requires it."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "pause", "reset", "set"],
                    "description": "Timer action.",
                },
                "duration_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "description": "Optional new duration; resets the timer.",
                },
            },
            "required": ["action"],
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
            "Read every persistent sticker on the video board, including "
            "countdowns, tomato timers, reminders, and todos."
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
            "Place a persistent sticker at board coordinates. Countdowns need a "
            "YYYY-MM-DD target; timers accept an optional duration; todos accept "
            "done; reminders are plain text. Stickers survive renderer restarts."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["countdown", "timer", "reminder", "todo"],
                },
                "x": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 960,
                    "description": "Left edge in board pixels.",
                },
                "y": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 540,
                    "description": "Top edge in board pixels.",
                },
                "text": {
                    "type": "string",
                    "maxLength": 160,
                    "description": "Label or body; required except for timer.",
                },
                "target": {
                    "type": "string",
                    "description": "YYYY-MM-DD date for a countdown.",
                },
                "duration_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3600,
                    "description": "Optional timer duration.",
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
            "Move or edit a sticker. Timers accept action (start, pause, reset, "
            "set) and duration_seconds; countdowns accept a new target; todos "
            "accept done."
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
    "bronzecast_set_open_item",
    "bronzecast_clear_open_item",
    "bronzecast_set_timer",
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


def _validate_open_item(arguments):
    title = arguments.get("title")
    note = arguments.get("note", "")
    if not isinstance(title, str) or not isinstance(note, str):
        raise ToolError("title and note must be strings")
    title = title.strip()
    note = note.strip()
    if not title:
        raise ToolError("title must not be empty")
    if len(title) > 48:
        raise ToolError("title must be 48 characters or fewer")
    if len(note) > 160:
        raise ToolError("note must be 160 characters or fewer")
    if any(_control(char) for char in title):
        raise ToolError("title must not contain control characters")
    if any(char != "\n" and _control(char) for char in note):
        raise ToolError("note only allows newlines as control characters")
    return {"title": title, "note": note}


def _validate_timer(arguments):
    action = arguments.get("action")
    if action not in {"start", "pause", "reset", "set"}:
        raise ToolError("action must be start, pause, reset, or set")
    duration = arguments.get("duration_seconds")
    if duration is not None:
        if isinstance(duration, bool) or not isinstance(duration, int):
            raise ToolError("duration_seconds must be an integer")
        if not 1 <= duration <= 3600:
            raise ToolError("duration_seconds must be between 1 and 3600")
    if action == "set" and duration is None:
        raise ToolError("action 'set' requires duration_seconds")
    payload = {"action": action}
    if duration is not None:
        payload["duration_seconds"] = duration
    return payload


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
    kind = arguments.get("kind")
    if kind not in STICKER_KINDS:
        raise ToolError("kind must be countdown, timer, reminder, or todo")
    payload = {
        "kind": kind,
        "x": _coordinate(arguments, "x", 960),
        "y": _coordinate(arguments, "y", 540),
    }
    text = _sticker_text(arguments.get("text"), required=kind != "timer")
    if text or kind == "timer":
        payload["text"] = text
    target = arguments.get("target")
    duration = arguments.get("duration_seconds")
    done = arguments.get("done", False)
    if not isinstance(done, bool):
        raise ToolError("done must be a boolean")
    if target is not None and kind != "countdown":
        raise ToolError("target only applies to countdown stickers")
    if duration is not None and kind != "timer":
        raise ToolError("duration_seconds only applies to timer stickers")
    if done and kind != "todo":
        raise ToolError("done only applies to todo stickers")
    if kind == "countdown":
        if target is None:
            raise ToolError("countdown requires target (YYYY-MM-DD)")
        payload["target"] = _sticker_date(target)
    elif kind == "timer" and duration is not None:
        payload["duration_seconds"] = _sticker_duration(duration)
    elif kind == "todo":
        payload["done"] = done
    return payload


def _validate_sticker_update(arguments):
    payload = {}
    for name, maximum in (("x", 960), ("y", 540)):
        if name in arguments:
            payload[name] = _coordinate(arguments, name, maximum)
    if "text" in arguments:
        payload["text"] = _sticker_text(
            arguments["text"], required=True, newlines=True
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
    if payload.get("action") == "set" and "duration_seconds" not in payload:
        raise ToolError("action 'set' requires duration_seconds")
    if not payload:
        raise ToolError("at least one sticker field must be provided")
    return payload


def _text(value):
    return json.dumps(value, indent=2, sort_keys=True)


async def _status_payload(settings):
    character, notifications, open_item, stickers, timer, stream = await asyncio.gather(
        _json("GET", settings.generator_url + "/api/character"),
        _json("GET", settings.generator_url + "/api/notifications"),
        _json("GET", settings.generator_url + "/api/open-item"),
        _json("GET", settings.generator_url + "/api/stickers"),
        _json("GET", settings.generator_url + "/api/timer"),
        _json("GET", settings.playout_url + "/health"),
        return_exceptions=True,
    )
    payload = {}
    for key, value in (
        ("character", character),
        ("notifications", notifications),
        ("open_item", open_item),
        ("stickers", stickers),
        ("timer", timer),
        ("stream", stream),
    ):
        payload[key] = {"error": str(value)} if isinstance(value, Exception) else value
    core_failed = isinstance(character, Exception) and isinstance(
        notifications, Exception
    )
    return payload, core_failed


async def call_tool(name, arguments, settings):
    if name == "bronzecast_get_status":
        payload, failed = await _status_payload(settings)
        return {"content": [{"type": "text", "text": _text(payload)}], "isError": failed}

    if name == "bronzecast_list_characters":
        return {"content": [{"type": "text", "text": _text(await _json("GET", settings.generator_url + "/api/character"))}]}

    if name == "bronzecast_get_timer":
        return {
            "content": [
                {
                    "type": "text",
                    "text": _text(
                        await _json("GET", settings.generator_url + "/api/timer")
                    ),
                }
            ]
        }

    if name == "bronzecast_set_timer":
        result = await _json(
            "POST",
            settings.generator_url + "/api/timer",
            _validate_timer(arguments),
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

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

    if name == "bronzecast_get_open_item":
        return {
            "content": [
                {
                    "type": "text",
                    "text": _text(
                        await _json(
                            "GET", settings.generator_url + "/api/open-item"
                        )
                    ),
                }
            ]
        }

    if name == "bronzecast_set_open_item":
        result = await _json(
            "PUT",
            settings.generator_url + "/api/open-item",
            _validate_open_item(arguments),
        )
        return {"content": [{"type": "text", "text": _text(result)}]}

    if name == "bronzecast_clear_open_item":
        await _json("DELETE", settings.generator_url + "/api/open-item")
        return {"content": [{"type": "text", "text": "Open item cleared."}]}

    raise ToolError(f"Unknown tool: {name}")
