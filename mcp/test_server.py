import base64
import hashlib
import json
import os
import re
import tempfile
import unittest
from urllib.parse import parse_qs, urlsplit
from unittest.mock import patch

from starlette.testclient import TestClient

from mcp import cimd
from mcp.config import Settings
from mcp.server import create_app
from mcp.tools import ToolError

REDIRECT = "http://127.0.0.1:9999/callback"
VERIFIER = "A" * 48


def challenge_for(verifier=VERIFIER):
    digest = hashlib.sha256(verifier.encode("ascii")).digest()
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")


class MCPTests(unittest.TestCase):
    def setUp(self):
        cimd.clear_cache()
        self.directory = tempfile.TemporaryDirectory()
        self.settings = Settings(
            public_base_url="https://mcp.test",
            db_path=os.path.join(self.directory.name, "mcp.sqlite3"),
            access_keys=("sekret",),
            generator_url="http://generator.test",
            playout_url="http://playout.test",
            tsx_api_url="http://tsx.test",
        )
        self.app = create_app(self.settings)
        self.client = TestClient(self.app, base_url="https://mcp.test")

    def tearDown(self):
        self.app.state.store.close()
        self.directory.cleanup()

    def register(self, redirect=REDIRECT):
        response = self.client.post(
            "/register",
            json={"client_name": "Test client", "redirect_uris": [redirect]},
        )
        self.assertEqual(response.status_code, 201, response.text)
        return response.json()["client_id"]

    def authorize_params(self, client_id, **overrides):
        params = {
            "response_type": "code",
            "client_id": client_id,
            "redirect_uri": REDIRECT,
            "state": "state-123",
            "code_challenge": challenge_for(),
            "code_challenge_method": "S256",
            "scope": "bronzecast.read bronzecast.write offline_access",
            "resource": "https://mcp.test/mcp",
        }
        params.update(overrides)
        return params

    def approve(self, params, key="sekret", client=None):
        client = client or self.client
        page = client.get("/authorize", params=params, follow_redirects=False)
        self.assertEqual(page.status_code, 200, page.text)
        match = re.search(r'name="csrf" value="([^"]+)"', page.text)
        self.assertIsNotNone(match)
        return client.post(
            "/authorize",
            data={**params, "csrf": match.group(1), "key": key, "decision": "approve"},
            follow_redirects=False,
        )

    def code_from(self, response):
        self.assertEqual(response.status_code, 303, response.text)
        location = response.headers["location"]
        return parse_qs(urlsplit(location).query)["code"][0]

    def tokens(self, scope="bronzecast.read bronzecast.write offline_access", client=None):
        client = client or self.client
        client_id = self.register()
        params = self.authorize_params(client_id, scope=scope)
        code = self.code_from(self.approve(params, client=client))
        response = client.post(
            "/token",
            data={
                "grant_type": "authorization_code",
                "code": code,
                "client_id": client_id,
                "redirect_uri": REDIRECT,
                "code_verifier": VERIFIER,
                "resource": "https://mcp.test/mcp",
            },
        )
        self.assertEqual(response.status_code, 200, response.text)
        return client_id, response.json()

    def mcp(self, access, payload, client=None):
        client = client or self.client
        return client.post(
            "/mcp",
            json={"jsonrpc": "2.0", **payload},
            headers={"Authorization": f"Bearer {access}"},
        )

    def test_discovery_advertises_dcr_and_cimd(self):
        metadata = self.client.get("/.well-known/oauth-authorization-server").json()
        self.assertTrue(metadata["client_id_metadata_document_supported"])
        self.assertIn("registration_endpoint", metadata)
        self.assertEqual(metadata["code_challenge_methods_supported"], ["S256"])
        resource = self.client.get("/.well-known/oauth-protected-resource").json()
        self.assertEqual(resource["resource"], "https://mcp.test/mcp")
        suffixed = self.client.get("/.well-known/oauth-protected-resource/mcp").json()
        self.assertEqual(suffixed["resource"], "https://mcp.test/mcp")

    def test_registration_rejects_unsafe_metadata(self):
        for body in (
            {"client_name": "No redirects"},
            {"redirect_uris": ["http://example.com/cb"]},
            {"redirect_uris": [REDIRECT], "token_endpoint_auth_method": "client_secret_basic"},
            {"redirect_uris": [REDIRECT], "response_types": ["token"]},
        ):
            response = self.client.post("/register", json=body)
            self.assertEqual(response.status_code, 400, body)

    def test_dcr_authorize_and_call_tools(self):
        client_id, tokens = self.tokens()
        self.assertIn("refresh_token", tokens)

        initialize = self.mcp(
            tokens["access_token"],
            {
                "id": 1,
                "method": "initialize",
                "params": {"protocolVersion": "2025-06-18"},
            },
        )
        self.assertEqual(initialize.status_code, 200)
        self.assertEqual(
            initialize.json()["result"]["protocolVersion"], "2025-06-18"
        )
        self.assertTrue(initialize.headers["mcp-session-id"])

        listed = self.mcp(tokens["access_token"], {"id": 2, "method": "tools/list"})
        names = {tool["name"] for tool in listed.json()["result"]["tools"]}
        self.assertIn("bronzecast_switch_character", names)
        self.assertNotIn("bronzecast_send_notification", names)
        self.assertNotIn("bronzecast_list_notifications", names)
        self.assertNotIn("bronzecast_clear_notifications", names)
        self.assertIn("bronzecast_get_snapshot", names)
        self.assertIn("bronzecast_list_stickers", names)
        self.assertIn("bronzecast_add_sticker", names)
        self.assertIn("bronzecast_update_sticker", names)
        self.assertIn("bronzecast_remove_sticker", names)
        self.assertIn("bronzecast_tsx_list_stickers", names)
        self.assertIn("bronzecast_tsx_get_sticker", names)
        self.assertIn("bronzecast_tsx_create_sticker", names)
        self.assertIn("bronzecast_tsx_update_sticker", names)
        self.assertIn("bronzecast_tsx_set_sticker_state", names)
        self.assertIn("bronzecast_tsx_remove_sticker", names)
        self.assertFalse(any("open_item" in name or name.endswith("_timer") for name in names))
        add_tool = next(
            tool for tool in listed.json()["result"]["tools"]
            if tool["name"] == "bronzecast_add_sticker"
        )
        self.assertEqual(len(add_tool["inputSchema"]["properties"]["appearance"]["enum"]), 25)
        self.assertIn("custom_svg", add_tool["inputSchema"]["properties"]["appearance"]["enum"])
        self.assertIn("svg", add_tool["inputSchema"]["properties"])
        self.assertIn("appearance", add_tool["inputSchema"]["required"])

        calls = []

        async def fake_json(method, url, payload=None):
            calls.append((method, url, payload))
            if url.endswith("/api/character") and method == "GET":
                return {"active_id": "mog", "requested_id": "mog", "characters": []}
            if url.endswith("/api/stickers") and method == "GET":
                return [{"id": 1, "kind": "timer", "x": 28, "y": 24, "angle": 0}]
            if url.endswith("/api/stickers") and method == "POST":
                return {"id": 2, **payload}
            if "/api/stickers/" in url and method == "PATCH":
                return {"id": int(url.rsplit("/", 1)[1]), **payload}
            if "/api/stickers/" in url and method == "DELETE":
                return None
            if url.endswith("/health"):
                return {"source": "live"}
            return {}

        with patch("mcp.tools._json", fake_json), patch(
            "mcp.tools._snapshot_png", return_value=b"fake-png"
        ):
            status = self.mcp(
                tokens["access_token"],
                {"id": 3, "method": "tools/call", "params": {"name": "bronzecast_get_status", "arguments": {}}},
            )
            self.assertFalse(status.json()["result"].get("isError", False))
            status_payload = json.loads(status.json()["result"]["content"][0]["text"])
            self.assertIn("stickers", status_payload)
            self.assertNotIn("notifications", status_payload)
            switch = self.mcp(
                tokens["access_token"],
                {
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_switch_character",
                        "arguments": {"id": "chocobo"},
                    },
                },
            )
            self.assertFalse(switch.json()["result"].get("isError", False))
            notification_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 6,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "notification",
                            "appearance": "ghost",
                            "x": 540,
                            "y": 80,
                            "title": "Kupo",
                            "text": "Mail!",
                            "expires_in_seconds": 3600,
                        },
                    },
                },
            )
            self.assertFalse(notification_sticker.json()["result"].get("isError", False))
            clock_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 15,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "clock",
                            "appearance": "sun",
                            "x": 240,
                            "y": 40,
                        },
                    },
                },
            )
            self.assertFalse(clock_sticker.json()["result"].get("isError", False))
            art_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 16,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "art",
                            "appearance": "custom_svg",
                            "x": 40,
                            "y": 40,
                            "svg": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 140"><circle cx="130" cy="70" r="50" fill="#e55"/></svg>',
                        },
                    },
                },
            )
            self.assertFalse(art_sticker.json()["result"].get("isError", False))
            add_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 7,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "countdown",
                            "appearance": "maple_leaf",
                            "x": 476,
                            "y": 380,
                            "text": "til Halloween",
                            "target": "2026-10-31",
                        },
                    },
                },
            )
            self.assertFalse(add_sticker.json()["result"].get("isError", False))
            list_stickers = self.mcp(
                tokens["access_token"],
                {
                    "id": 8,
                    "method": "tools/call",
                    "params": {"name": "bronzecast_list_stickers", "arguments": {}},
                },
            )
            self.assertFalse(list_stickers.json()["result"].get("isError", False))
            update_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 9,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_update_sticker",
                        "arguments": {
                            "id": 2,
                            "appearance": "christmas_tree",
                            "x": 500,
                            "y": 400,
                            "angle": 22.5,
                        },
                    },
                },
            )
            self.assertFalse(update_sticker.json()["result"].get("isError", False))
            remove_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 10,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_remove_sticker",
                        "arguments": {"id": 2},
                    },
                },
            )
            self.assertFalse(remove_sticker.json()["result"].get("isError", False))
            memo_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 12,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "memo",
                            "appearance": "ghost",
                            "x": 620,
                            "y": 80,
                            "angle": -12,
                            "title": "Plan",
                            "text": "A movable memo",
                        },
                    },
                },
            )
            self.assertFalse(memo_sticker.json()["result"].get("isError", False))
            timer_action = self.mcp(
                tokens["access_token"],
                {
                    "id": 13,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_update_sticker",
                        "arguments": {"id": 1, "action": "set", "duration_seconds": 900},
                    },
                },
            )
            self.assertFalse(timer_action.json()["result"].get("isError", False))
            timer_sticker = self.mcp(
                tokens["access_token"],
                {
                    "id": 14,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_add_sticker",
                        "arguments": {
                            "kind": "timer",
                            "appearance": "pumpkin_pie",
                            "x": 20,
                            "y": 20,
                            "duration_seconds": 1200,
                        },
                    },
                },
            )
            self.assertFalse(timer_sticker.json()["result"].get("isError", False))
            snapshot = self.mcp(
                tokens["access_token"],
                {
                    "id": 11,
                    "method": "tools/call",
                    "params": {"name": "bronzecast_get_snapshot", "arguments": {}},
                },
            )
            self.assertFalse(snapshot.json()["result"].get("isError", False))
            image_content = snapshot.json()["result"]["content"][0]
            self.assertEqual(image_content["type"], "image")
            self.assertEqual(image_content["mimeType"], "image/png")
            self.assertEqual(base64.b64decode(image_content["data"]), b"fake-png")
        self.assertIn(
            ("POST", "http://generator.test/api/character", {"id": "chocobo"}), calls
        )
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {
                    "kind": "notification",
                    "appearance": "ghost",
                    "x": 540,
                    "y": 80,
                    "title": "Kupo",
                    "text": "Mail!",
                    "expires_in_seconds": 3600,
                },
            ),
            calls,
        )
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {"kind": "clock", "appearance": "sun", "x": 240, "y": 40},
            ),
            calls,
        )
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {
                    "kind": "art",
                    "appearance": "custom_svg",
                    "x": 40,
                    "y": 40,
                    "svg": '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 140"><circle cx="130" cy="70" r="50" fill="#e55"/></svg>',
                },
            ),
            calls,
        )
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {
                    "kind": "countdown",
                    "appearance": "maple_leaf",
                    "text": "til Halloween",
                    "x": 476,
                    "y": 380,
                    "target": "2026-10-31",
                },
            ),
            calls,
        )
        self.assertIn(("GET", "http://generator.test/api/stickers", None), calls)
        self.assertIn(
            (
                "PATCH",
                "http://generator.test/api/stickers/2",
                {"appearance": "christmas_tree", "x": 500, "y": 400, "angle": 22.5},
            ),
            calls,
        )
        self.assertIn(("DELETE", "http://generator.test/api/stickers/2", None), calls)
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {"kind": "memo", "appearance": "ghost", "x": 620, "y": 80, "angle": -12, "title": "Plan", "text": "A movable memo"},
            ),
            calls,
        )
        self.assertIn(
            (
                "PATCH",
                "http://generator.test/api/stickers/1",
                {"action": "set", "duration_seconds": 900},
            ),
            calls,
        )
        self.assertIn(
            (
                "POST",
                "http://generator.test/api/stickers",
                {"kind": "timer", "appearance": "pumpkin_pie", "x": 20, "y": 20, "duration_seconds": 1200},
            ),
            calls,
        )

    def test_tsx_tools_create_component_and_replace_whole_shared_state(self):
        _, tokens = self.tokens()
        calls = []
        component = (
            "export const editableProps = { complete: { type: 'boolean', default: false } }; "
            "export default function Todo({state}) { return <div>{state.complete ? 'DONE' : 'OPEN'}</div> }"
        )
        record = {
            "id": 41,
            "tsx": component,
            "x": 40,
            "y": 60,
            "angle": 0,
            "schemaRegistered": True,
            "editableProps": {"complete": {"type": "boolean", "default": False}},
            "state": {"complete": False},
        }

        async def fake_json(method, url, payload=None):
            calls.append((method, url, payload))
            if method == "POST" and url == "http://tsx.test/api/stickers":
                return {**record, "schemaRegistered": False, "state": payload.get("initialState", {})}
            if method == "GET" and url == "http://tsx.test/api/stickers/41":
                return record
            if method == "PUT" and url == "http://tsx.test/api/stickers/41/state":
                record["state"] = payload
                return record
            if method == "GET" and url == "http://tsx.test/api/stickers":
                return [record]
            return {}

        with patch("mcp.tools._json", fake_json):
            created = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_tsx_create_sticker",
                        "arguments": {
                            "tsx": component,
                            "x": 40,
                            "y": 60,
                            "initial_state": {"complete": False},
                        },
                    },
                },
            )
            self.assertFalse(created.json()["result"].get("isError", False))
            created_value = json.loads(created.json()["result"]["content"][0]["text"])
            self.assertEqual(created_value["id"], 41)
            self.assertFalse(created_value["schemaPending"])

            written = self.mcp(
                tokens["access_token"],
                {
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "bronzecast_tsx_set_sticker_state",
                        "arguments": {"id": 41, "state": {"complete": True}},
                    },
                },
            )
            self.assertFalse(written.json()["result"].get("isError", False))
            written_value = json.loads(written.json()["result"]["content"][0]["text"])
            self.assertEqual(written_value["state"], {"complete": True})

        self.assertIn(
            (
                "POST",
                "http://tsx.test/api/stickers",
                {
                    "tsx": component,
                    "x": 40,
                    "y": 60,
                    "angle": 0,
                    "initialState": {"complete": False},
                },
            ),
            calls,
        )
        self.assertIn(
            ("PUT", "http://tsx.test/api/stickers/41/state", {"complete": True}),
            calls,
        )

    def test_sticker_validation(self):
        _, tokens = self.tokens()
        invalid = (
            ("bronzecast_add_sticker", {"kind": "countdown", "appearance": "maple_leaf", "x": 1, "y": 1, "text": "x"}),
            ("bronzecast_add_sticker", {"kind": "countdown", "appearance": "maple_leaf", "x": 1, "y": 1, "text": "x", "target": "10-31-2026"}),
            ("bronzecast_add_sticker", {"kind": "timer", "appearance": "pumpkin_pie", "x": 1, "y": 1, "target": "2026-10-31"}),
            ("bronzecast_add_sticker", {"kind": "reminder", "x": 1, "y": 1, "text": "x", "done": True}),
            ("bronzecast_add_sticker", {"kind": "note", "x": 1, "y": 1, "text": "x"}),
            ("bronzecast_add_sticker", {"kind": "todo", "appearance": "turkey", "x": 1, "y": 1, "text": "x", "duration_seconds": 60}),
            ("bronzecast_add_sticker", {"kind": "todo", "x": 1, "y": 1, "text": "x"}),
            ("bronzecast_add_sticker", {"kind": "todo", "appearance": "unicorn", "x": 1, "y": 1, "text": "x"}),
            ("bronzecast_add_sticker", {"kind": "memo", "appearance": "ghost", "x": 1, "y": 1}),
            ("bronzecast_add_sticker", {"kind": "memo", "appearance": "ghost", "x": 1, "y": 1, "title": "", "text": "  "}),
            ("bronzecast_add_sticker", {"kind": "todo", "x": 1, "y": 1, "text": "Task"}),
            ("bronzecast_add_sticker", {"kind": "todo", "appearance": "unicorn", "x": 1, "y": 1, "text": "Task"}),
            ("bronzecast_add_sticker", {"kind": "notification", "appearance": "ghost", "x": 1, "y": 1, "text": "Message"}),
            ("bronzecast_add_sticker", {"kind": "notification", "appearance": "ghost", "x": 1, "y": 1, "text": "Message", "expires_in_seconds": 0}),
            ("bronzecast_add_sticker", {"kind": "notification", "appearance": "ghost", "x": 1, "y": 1, "text": "Message", "expires_in_seconds": True}),
            ("bronzecast_add_sticker", {"kind": "notification", "appearance": "ghost", "x": 1, "y": 1, "text": "Message", "expires_in_seconds": 30, "title": "  "}),
            ("bronzecast_add_sticker", {"kind": "clock", "appearance": "sun", "x": 1, "y": 1, "text": "Static text"}),
            ("bronzecast_add_sticker", {"kind": "art", "appearance": "custom_svg", "x": 1, "y": 1}),
            ("bronzecast_add_sticker", {"kind": "art", "appearance": "ghost", "x": 1, "y": 1, "svg": "<svg/>"}),
            ("bronzecast_add_sticker", {"kind": "art", "appearance": "custom_svg", "x": 1, "y": 1, "svg": "<svg/>", "text": "Rendered by SVG"}),
            ("bronzecast_add_sticker", {"kind": "todo", "appearance": "custom_svg", "x": 1, "y": 1, "text": "Task"}),
            ("bronzecast_add_sticker", {"kind": "todo", "appearance": "ghost", "x": 1, "y": 1, "text": "Task", "svg": "<svg/>"}),
            ("bronzecast_add_sticker", {"kind": "reminder", "x": 961, "y": 1, "text": "x"}),
            ("bronzecast_update_sticker", {"id": 0, "x": 1}),
            ("bronzecast_update_sticker", {"id": 1}),
            ("bronzecast_update_sticker", {"id": 1, "action": "dance"}),
            ("bronzecast_update_sticker", {"id": 1, "action": "set"}),
            ("bronzecast_remove_sticker", {"id": -1}),
        )
        for name, arguments in invalid:
            response = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "tools/call",
                    "params": {"name": name, "arguments": arguments},
                },
            )
            self.assertTrue(response.json()["result"]["isError"], (name, arguments))

    def test_timer_sticker_validation(self):
        _, tokens = self.tokens()
        for arguments in (
            {"kind": "timer", "appearance": "pumpkin_pie", "x": 1, "y": 1},
            {"kind": "timer", "appearance": "pumpkin_pie", "x": 1, "y": 1, "duration_seconds": 0},
            {"kind": "timer", "appearance": "pumpkin_pie", "x": 1, "y": 1, "duration_seconds": 3601},
            {"kind": "timer", "appearance": "pumpkin_pie", "x": 1, "y": 1, "duration_seconds": True},
            {"id": 1, "action": "dance"},
            {"id": 1, "action": "set"},
            {"id": 1, "action": "start", "duration_seconds": 60},
        ):
            tool_name = "bronzecast_add_sticker" if "kind" in arguments else "bronzecast_update_sticker"
            response = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": tool_name,
                        "arguments": arguments,
                    },
                },
            )
            self.assertTrue(response.json()["result"]["isError"], arguments)

    def test_cimd_client_id_is_fetched_and_validated(self):
        client_id = "https://client.example/metadata.json"
        document = {
            "client_id": client_id,
            "client_name": "CIMD client",
            "redirect_uris": ["https://client.example/callback"],
            "token_endpoint_auth_method": "none",
        }

        def fetcher(url):
            self.assertEqual(url, client_id)
            return {"content-type": "application/json"}, json.dumps(document).encode()

        app = create_app(self.settings, client_fetcher=fetcher)
        client = TestClient(app, base_url="https://mcp.test")
        params = self.authorize_params(client_id, redirect_uri="https://client.example/callback")
        response = self.approve(params, client=client)
        self.assertEqual(response.status_code, 303, response.text)
        code = parse_qs(urlsplit(response.headers["location"]).query)["code"][0]
        token = client.post(
            "/token",
            data={
                "grant_type": "authorization_code",
                "code": code,
                "client_id": client_id,
                "redirect_uri": "https://client.example/callback",
                "code_verifier": VERIFIER,
            },
        )
        self.assertEqual(token.status_code, 200, token.text)
        app.state.store.close()

    def test_cimd_document_must_repeat_client_id(self):
        client_id = "https://client.example/metadata.json"

        def fetcher(url):
            return {"content-type": "application/json"}, json.dumps(
                {
                    "client_id": "https://client.example/other.json",
                    "redirect_uris": ["https://client.example/callback"],
                }
            ).encode()

        app = create_app(self.settings, client_fetcher=fetcher)
        client = TestClient(app, base_url="https://mcp.test")
        params = self.authorize_params(client_id, redirect_uri="https://client.example/callback")
        response = client.get("/authorize", params=params, follow_redirects=False)
        self.assertEqual(response.status_code, 400)
        app.state.store.close()

    def test_cimd_rejects_private_hosts(self):
        for host in ("127.0.0.1", "localhost", "10.0.0.5"):
            with self.assertRaises(cimd.ClientMetadataError):
                cimd.ensure_public_host(host)
        with self.assertRaises(cimd.ClientMetadataError):
            cimd.validate_client_id("http://client.example/metadata.json")

    def test_wrong_key_is_rejected_and_rate_limited(self):
        client_id = self.register()
        params = self.authorize_params(client_id)
        page = self.client.get("/authorize", params=params)
        csrf = re.search(r'name="csrf" value="([^"]+)"', page.text).group(1)
        for _ in range(5):
            response = self.client.post(
                "/authorize",
                data={**params, "csrf": csrf, "key": "wrong", "decision": "approve"},
                follow_redirects=False,
            )
            self.assertEqual(response.status_code, 401)
        blocked = self.client.post(
            "/authorize",
            data={**params, "csrf": csrf, "key": "sekret", "decision": "approve"},
            follow_redirects=False,
        )
        self.assertEqual(blocked.status_code, 429)

    def test_redirect_mismatch_never_redirects(self):
        client_id = self.register()
        params = self.authorize_params(client_id, redirect_uri="https://evil.example/cb")
        response = self.client.get("/authorize", params=params, follow_redirects=False)
        self.assertEqual(response.status_code, 400)
        self.assertNotIn("location", response.headers)

    def test_redirectable_errors_carry_state_and_issuer(self):
        client_id = self.register()
        params = self.authorize_params(client_id, response_type="token")
        response = self.client.get("/authorize", params=params, follow_redirects=False)
        self.assertEqual(response.status_code, 303)
        query = parse_qs(urlsplit(response.headers["location"]).query)
        self.assertEqual(query["error"][0], "unsupported_response_type")
        self.assertEqual(query["state"][0], "state-123")
        self.assertEqual(query["iss"][0], "https://mcp.test")

    def test_pkce_failure_and_single_use_codes(self):
        client_id = self.register()
        code = self.code_from(self.approve(self.authorize_params(client_id)))
        wrong = self.client.post(
            "/token",
            data={
                "grant_type": "authorization_code",
                "code": code,
                "client_id": client_id,
                "redirect_uri": REDIRECT,
                "code_verifier": "B" * 48,
            },
        )
        self.assertEqual(wrong.status_code, 400)
        self.assertEqual(wrong.json()["error"], "invalid_grant")

        code = self.code_from(self.approve(self.authorize_params(client_id)))
        first = self.client.post(
            "/token",
            data={
                "grant_type": "authorization_code",
                "code": code,
                "client_id": client_id,
                "redirect_uri": REDIRECT,
                "code_verifier": VERIFIER,
            },
        )
        self.assertEqual(first.status_code, 200)
        second = self.client.post(
            "/token",
            data={
                "grant_type": "authorization_code",
                "code": code,
                "client_id": client_id,
                "redirect_uri": REDIRECT,
                "code_verifier": VERIFIER,
            },
        )
        self.assertEqual(second.status_code, 400)

    def test_refresh_rotation_and_reuse_revokes_family(self):
        client_id, tokens = self.tokens()
        rotated = self.client.post(
            "/token",
            data={
                "grant_type": "refresh_token",
                "refresh_token": tokens["refresh_token"],
                "client_id": client_id,
            },
        )
        self.assertEqual(rotated.status_code, 200, rotated.text)
        replacement = rotated.json()["refresh_token"]
        self.assertNotEqual(replacement, tokens["refresh_token"])

        reused = self.client.post(
            "/token",
            data={
                "grant_type": "refresh_token",
                "refresh_token": tokens["refresh_token"],
                "client_id": client_id,
            },
        )
        self.assertEqual(reused.status_code, 400)
        after = self.client.post(
            "/token",
            data={
                "grant_type": "refresh_token",
                "refresh_token": replacement,
                "client_id": client_id,
            },
        )
        self.assertEqual(after.status_code, 400)

    def test_scopes_gate_write_tools(self):
        _, tokens = self.tokens(scope="bronzecast.read")
        self.assertNotIn("refresh_token", tokens)
        denied = self.mcp(
            tokens["access_token"],
            {
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "bronzecast_switch_character",
                    "arguments": {"id": "chocobo"},
                },
            },
        )
        self.assertEqual(denied.status_code, 403)
        self.assertIn("insufficient_scope", denied.headers["www-authenticate"])

        async def fake_json(method, url, payload=None):
            return {"active_id": "mog", "requested_id": "mog", "characters": []}

        with patch("mcp.tools._json", fake_json):
            allowed = self.mcp(
                tokens["access_token"],
                {
                    "id": 2,
                    "method": "tools/call",
                    "params": {"name": "bronzecast_list_characters", "arguments": {}},
                },
            )
        self.assertEqual(allowed.status_code, 200)
        self.assertFalse(allowed.json()["result"].get("isError", False))

    def test_bearer_required_revocation_and_audience(self):
        missing = self.client.post(
            "/mcp", json={"jsonrpc": "2.0", "id": 1, "method": "ping"}
        )
        self.assertEqual(missing.status_code, 401)
        self.assertIn("resource_metadata", missing.headers["www-authenticate"])

        _, tokens = self.tokens()
        self.client.post("/revoke", data={"token": tokens["access_token"]})
        revoked = self.mcp(tokens["access_token"], {"id": 1, "method": "ping"})
        self.assertEqual(revoked.status_code, 401)

        client_id = self.register()
        params = self.authorize_params(client_id, resource="https://elsewhere.example/mcp")
        response = self.client.get("/authorize", params=params, follow_redirects=False)
        self.assertEqual(response.status_code, 303)
        query = parse_qs(urlsplit(response.headers["location"]).query)
        self.assertEqual(query["error"][0], "invalid_target")

    def test_tool_errors_are_reported_not_raised(self):
        _, tokens = self.tokens()

        async def failing_json(method, url, payload=None):
            raise ToolError("generator offline")

        with patch("mcp.tools._json", failing_json):
            response = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "tools/call",
                    "params": {"name": "bronzecast_list_stickers", "arguments": {}},
                },
            )
        self.assertEqual(response.status_code, 200)
        self.assertTrue(response.json()["result"]["isError"])

    def test_snapshot_is_read_scope_image_content(self):
        _, tokens = self.tokens(scope="bronzecast.read")
        png = b"\x89PNG\r\n\x1a\nboard"
        with patch("mcp.tools._snapshot_png", return_value=png):
            response = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "tools/call",
                    "params": {"name": "bronzecast_get_snapshot", "arguments": {}},
                },
            )
        content = response.json()["result"]["content"][0]
        self.assertEqual(content["type"], "image")
        self.assertEqual(content["mimeType"], "image/png")
        self.assertEqual(base64.b64decode(content["data"]), png)

    def test_resources_read_snapshot(self):
        _, tokens = self.tokens()

        async def fake_json(method, url, payload=None):
            if url.endswith("/api/character"):
                return {"active_id": "mog"}
            if url.endswith("/api/stickers"):
                return [{"kind": "timer", "state": "running", "remaining_seconds": 1490}]
            return {"source": "live"}

        with patch("mcp.tools._json", fake_json):
            response = self.mcp(
                tokens["access_token"],
                {
                    "id": 1,
                    "method": "resources/read",
                    "params": {"uri": "bronzecast://dashboard/state"},
                },
            )
        contents = response.json()["result"]["contents"]
        self.assertEqual(contents[0]["uri"], "bronzecast://dashboard/state")
        self.assertIn('"source": "live"', contents[0]["text"])
        self.assertIn('"state": "running"', contents[0]["text"])


if __name__ == "__main__":
    unittest.main()
