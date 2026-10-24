"""MCP tool definitions and dispatch onto the renderer/playout HTTP APIs."""
import asyncio
import json
import re

import httpx

TIMEOUT = 5.0
CHARACTER_ID_RE = re.compile(r"^[^\s\x00-\x1f\x7f]{1,64}$")


class ToolError(Exception):
    pass


TOOLS = [
    {
        "name": "bronzecast_get_status",
        "title": "Get dashboard status",
        "description": (
            "Read the active character, the persistent open item, the "
            "notification queue, and the live playout encoder state in one call."
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


def _text(value):
    return json.dumps(value, indent=2, sort_keys=True)


async def _status_payload(settings):
    character, notifications, open_item, stream = await asyncio.gather(
        _json("GET", settings.generator_url + "/api/character"),
        _json("GET", settings.generator_url + "/api/notifications"),
        _json("GET", settings.generator_url + "/api/open-item"),
        _json("GET", settings.playout_url + "/health"),
        return_exceptions=True,
    )
    payload = {}
    for key, value in (
        ("character", character),
        ("notifications", notifications),
        ("open_item", open_item),
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
