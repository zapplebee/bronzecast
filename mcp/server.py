"""BRONZECAST MCP server: OAuth authorization server + Streamable HTTP endpoint.

The authorization page deliberately asks for a statically configured key. OAuth
still does the protocol work MCP clients require (DCR/CIMD, PKCE, audience-bound
tokens, refresh rotation); the key is the human gate in front of it.
"""
import html
import json
import logging
import os
import secrets
import threading
import time
import uuid
from urllib.parse import parse_qs, urlsplit

import uvicorn
from starlette.applications import Starlette
from starlette.requests import Request
from starlette.responses import (
    HTMLResponse,
    JSONResponse,
    PlainTextResponse,
    RedirectResponse,
    Response,
)
from starlette.routing import Route

from . import oauth, tools
from .config import Settings
from .storage import Store

LOG = logging.getLogger("bronzecast.mcp")
SUPPORTED_PROTOCOLS = ("2025-11-25", "2025-06-18")
SESSION_HEADER = "Mcp-Session-Id"
CSRF_COOKIE = "bc_csrf"


class LoginLimiter:
    def __init__(self, limit=5, window=300):
        self.limit = limit
        self.window = window
        self._lock = threading.Lock()
        self._failures = {}

    def blocked(self, key):
        now = time.monotonic()
        with self._lock:
            recent = [stamp for stamp in self._failures.get(key, []) if stamp > now - self.window]
            self._failures[key] = recent
            return len(recent) >= self.limit

    def record_failure(self, key):
        with self._lock:
            self._failures.setdefault(key, []).append(time.monotonic())


def _form_body(body):
    return parse_qs(body.decode("utf-8", "replace"), keep_blank_values=True)


def _first(form, name, default=""):
    values = form.get(name)
    return values[0] if values else default


def _client_ip(request):
    # Traefik is the only ingress and sets X-Forwarded-For; otherwise use the peer.
    forwarded = request.headers.get("x-forwarded-for", "")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"


def _escape(value):
    return html.escape(value or "", quote=True)


def _scope_summary(scope):
    labels = {
        "bronzecast.read": "Read the character, notifications and stream state",
        "bronzecast.write": "Change the character and manage notifications",
        "offline_access": "Stay signed in with a refresh token",
    }
    return [(value, labels.get(value, value)) for value in scope.split()]


def _authorize_page(settings, context, csrf, error="", status=200):
    redirect_host = urlsplit(context.redirect_uri).hostname
    scopes = "".join(
        f"<li>{_escape(value)}<span>{_escape(label)}</span></li>"
        for value, label in _scope_summary(context.scope)
    )
    hidden = "".join(
        f'<input type="hidden" name="{name}" value="{_escape(value)}">'
        for name, value in (
            ("response_type", "code"),
            ("client_id", context.client_id),
            ("redirect_uri", context.redirect_uri),
            ("state", context.state),
            ("code_challenge", context.challenge),
            ("code_challenge_method", "S256"),
            ("scope", context.scope),
            ("resource", context.resource),
            ("csrf", csrf),
        )
    )
    notice = f'<p class="error">{_escape(error)}</p>' if error else ""
    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>BRONZECAST / AUTHORIZE</title>
<style>
:root {{ color-scheme: dark; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center;
  background: #0b0d0a; color: #c9d4b9; font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }}
main {{ width: min(460px, 92vw); border: 1px solid #2d3a24; background: #11150f;
  padding: 28px; box-shadow: 0 0 0 1px #060805, 0 24px 60px rgba(0,0,0,.6); }}
.kicker {{ margin: 0 0 10px; color: #8aa26f; font-size: 11px; letter-spacing: .18em; }}
h1 {{ margin: 0 0 6px; font-size: 20px; color: #edf3e2; font-weight: 600; }}
.redirect {{ margin: 0 0 18px; color: #8b9a7b; font-size: 12px; }}
.redirect code {{ color: #b9c9a3; }}
ul {{ list-style: none; margin: 0 0 20px; padding: 0; border-top: 1px solid #232d1c; }}
li {{ display: flex; justify-content: space-between; gap: 12px; padding: 9px 0;
  border-bottom: 1px solid #232d1c; color: #b7c6a5; font-size: 12px; }}
li span {{ color: #7d8c6d; text-align: right; }}
label {{ display: block; margin-bottom: 7px; color: #8aa26f; font-size: 11px; letter-spacing: .12em; }}
input[type=password] {{ width: 100%; padding: 11px 12px; margin-bottom: 18px;
  border: 1px solid #33422a; background: #0a0d08; color: #e6eeda; font: inherit; }}
input[type=password]:focus {{ outline: 1px solid #7d9a5c; }}
.actions {{ display: flex; gap: 10px; }}
button {{ flex: 1; padding: 11px 12px; border: 1px solid #496237; background: #1c2617;
  color: #dbe8c9; font: inherit; letter-spacing: .08em; cursor: pointer; }}
button.primary {{ background: #8fa96d; border-color: #a6c080; color: #10160c; font-weight: 700; }}
button:hover {{ filter: brightness(1.12); }}
.error {{ margin: 0 0 16px; padding: 9px 11px; border: 1px solid #6b3434;
  background: #221010; color: #e6a9a9; font-size: 12px; }}
.foot {{ margin: 18px 0 0; color: #58654c; font-size: 11px; }}
</style></head>
<body><main>
<p class="kicker">BRONZECAST / AUTHORIZATION</p>
<h1>{_escape(context.client_name)}</h1>
<p class="redirect">wants to connect to the live dashboard and return to <code>{_escape(redirect_host)}</code></p>
{notice}
<ul>{scopes}</ul>
<form method="post" action="/authorize">
{hidden}
<label for="key">ACCESS KEY</label>
<input id="key" name="key" type="password" autocomplete="off" autofocus required>
<div class="actions">
<button class="primary" type="submit" name="decision" value="approve">APPROVE</button>
<button type="submit" name="decision" value="cancel" formnovalidate>CANCEL</button>
</div>
</form>
<p class="foot">Paste the key configured for this MCP endpoint. Nothing else is stored about you.</p>
</main></body></html>"""
    response = HTMLResponse(page, status_code=status)
    response.set_cookie(
        CSRF_COOKIE,
        csrf,
        max_age=600,
        httponly=True,
        samesite="lax",
        secure=settings.issuer.startswith("https"),
        path="/",
    )
    return response


def _error_page(status, title, detail):
    page = f"""<!doctype html><meta charset="utf-8">
<title>BRONZECAST / {_escape(title)}</title>
<style>body{{background:#0b0d0a;color:#c9d4b9;font:14px/1.6 ui-monospace,Menlo,monospace;
display:grid;place-items:center;min-height:100vh;margin:0}}
main{{max-width:520px;border:1px solid #2d3a24;background:#11150f;padding:28px}}
h1{{font-size:18px;color:#edf3e2;margin:0 0 10px}}p{{color:#8b9a7b}}</style>
<main><h1>{_escape(title)}</h1><p>{_escape(detail)}</p></main>"""
    return HTMLResponse(page, status_code=status)


def _oauth_json(payload, status=200):
    return JSONResponse(
        payload,
        status_code=status,
        headers={"Cache-Control": "no-store", "Pragma": "no-cache"},
    )


def create_app(settings=None, client_fetcher=None):
    settings = settings or Settings.from_env()
    store = Store(settings.db_path)
    login_limiter = LoginLimiter(limit=5, window=300)
    register_limiter = LoginLimiter(limit=20, window=3600)
    sessions = set()
    sessions_lock = threading.Lock()

    def fail_authorize(redirect_uri, state, error, description):
        return RedirectResponse(
            oauth.redirect_url(
                redirect_uri,
                {
                    "error": error,
                    "error_description": description,
                    "state": state or None,
                    "iss": settings.issuer,
                },
            ),
            status_code=303,
        )

    def authorize_context(params):
        client = oauth.resolve_client(store, params.get("client_id"), client_fetcher)
        return oauth.build_authorize_context(params, client, settings)

    async def metadata(request):
        return JSONResponse(oauth.authorization_server_metadata(settings))

    async def resource_metadata(request):
        return JSONResponse(oauth.protected_resource_metadata(settings))

    async def register(request):
        if register_limiter.blocked(_client_ip(request)):
            return _oauth_json(
                {"error": "temporarily_unavailable", "error_description": "slow down"},
                status=429,
            )
        try:
            document = await request.json()
        except (json.JSONDecodeError, UnicodeDecodeError):
            return _oauth_json(
                {"error": "invalid_client_metadata", "error_description": "body must be JSON"},
                status=400,
            )
        if store.client_count() >= 1000:
            return _oauth_json(
                {"error": "temporarily_unavailable", "error_description": "client limit reached"},
                status=429,
            )
        try:
            body = oauth.register_client(store, document)
        except oauth.OAuthError as error:
            register_limiter.record_failure(_client_ip(request))
            return _oauth_json(
                {"error": error.error, "error_description": error.description}, status=400
            )
        return JSONResponse(body, status_code=201)

    async def authorize_get(request):
        params = request.query_params
        try:
            context = authorize_context(params)
        except oauth.ClientError as error:
            return _error_page(400, "Authorization request rejected", error.description)
        except oauth.OAuthError as error:
            return fail_authorize(
                params.get("redirect_uri"),
                params.get("state"),
                error.error,
                error.description,
            )
        csrf = secrets.token_urlsafe(24)
        return _authorize_page(settings, context, csrf)

    async def authorize_post(request):
        body = await request.body()
        form = _form_body(body)
        params = {
            name: _first(form, name)
            for name in (
                "response_type",
                "client_id",
                "redirect_uri",
                "state",
                "code_challenge",
                "code_challenge_method",
                "scope",
                "resource",
            )
        }
        cookie = request.cookies.get(CSRF_COOKIE, "")
        supplied_csrf = _first(form, "csrf")
        if not cookie or not supplied_csrf or not secrets.compare_digest(cookie, supplied_csrf):
            return _error_page(
                400,
                "Authorization form expired",
                "Enable cookies and start the sign-in flow again.",
            )
        try:
            context = authorize_context(params)
        except oauth.ClientError as error:
            return _error_page(400, "Authorization request rejected", error.description)
        except oauth.OAuthError as error:
            return fail_authorize(
                params.get("redirect_uri"),
                params.get("state"),
                error.error,
                error.description,
            )

        if _first(form, "decision") == "cancel":
            return RedirectResponse(
                oauth.redirect_url(
                    context.redirect_uri,
                    {
                        "error": "access_denied",
                        "error_description": "The user declined access",
                        "state": context.state or None,
                        "iss": settings.issuer,
                    },
                ),
                status_code=303,
            )

        if not settings.access_keys:
            return _error_page(
                500,
                "Endpoint not configured",
                "Set BRONZECAST_MCP_ACCESS_KEYS before authorizing clients.",
            )
        ip = _client_ip(request)
        if login_limiter.blocked(ip):
            return _error_page(
                429,
                "Too many attempts",
                "Wait a few minutes before trying the key again.",
            )
        if not oauth.key_matches(settings, _first(form, "key")):
            login_limiter.record_failure(ip)
            return _authorize_page(
                settings, context, supplied_csrf, error="That key is not valid.", status=401
            )

        code = store.create_code(
            context.client_id,
            context.redirect_uri,
            context.scope,
            context.resource,
            context.challenge,
            settings.code_ttl,
        )
        location = oauth.redirect_url(
            context.redirect_uri,
            {"code": code, "state": context.state or None, "iss": settings.issuer},
        )
        return RedirectResponse(location, status_code=303)

    async def token(request):
        media_type = request.headers.get("content-type", "").split(";")[0].strip().lower()
        if media_type != "application/x-www-form-urlencoded":
            return _oauth_json(
                {"error": "invalid_request", "error_description": "use form encoding"}, 400
            )
        form = {
            key: values[0]
            for key, values in _form_body(await request.body()).items()
        }
        try:
            return _oauth_json(oauth.exchange_token_request(settings, store, form))
        except oauth.OAuthError as error:
            return _oauth_json(
                {"error": error.error, "error_description": error.description},
                status=error.status,
            )

    async def revoke(request):
        form = {
            key: values[0]
            for key, values in _form_body(await request.body()).items()
        }
        if form.get("token"):
            store.revoke(form["token"])
        return Response(status_code=200)

    def unauthorized(error=None, description=None, scope=None):
        params = [
            f'resource_metadata="{settings.issuer}/.well-known/oauth-protected-resource"'
        ]
        if error:
            params.append(f'error="{error}"')
        if description:
            params.append(f'error_description="{description}"')
        if scope:
            params.append(f'scope="{scope}"')
        payload = {"error": error or "invalid_token"}
        if description:
            payload["error_description"] = description
        return JSONResponse(
            payload,
            status_code=403 if error == "insufficient_scope" else 401,
            headers={"WWW-Authenticate": "Bearer " + ", ".join(params)},
        )

    def rpc_result(request_id, result, headers=None):
        return JSONResponse(
            {"jsonrpc": "2.0", "id": request_id, "result": result}, headers=headers
        )

    def rpc_error(request_id, code, message):
        return JSONResponse(
            {"jsonrpc": "2.0", "id": request_id, "error": {"code": code, "message": message}}
        )

    async def mcp(request):
        token_record = oauth.token_from_request(
            settings, store, request.headers.get("authorization")
        )
        if token_record is None:
            return unauthorized("invalid_token", "A valid access token is required")
        if request.method == "GET":
            return Response(status_code=405, headers={"Allow": "POST, DELETE"})
        if request.method == "DELETE":
            with sessions_lock:
                sessions.discard(request.headers.get(SESSION_HEADER))
            return Response(status_code=204)

        version = request.headers.get("mcp-protocol-version")
        if version and version not in SUPPORTED_PROTOCOLS:
            return Response(status_code=400, headers={"MCP-Protocol-Version": SUPPORTED_PROTOCOLS[0]})
        try:
            payload = await request.json()
        except (json.JSONDecodeError, UnicodeDecodeError):
            return rpc_error(None, -32700, "Parse error")
        if isinstance(payload, list):
            return rpc_error(None, -32600, "Batch requests are not supported")
        if not isinstance(payload, dict):
            return rpc_error(None, -32600, "Invalid request")
        method = payload.get("method")
        request_id = payload.get("id")
        params = payload.get("params") or {}

        if method == "initialize":
            requested = params.get("protocolVersion")
            protocol = requested if requested in SUPPORTED_PROTOCOLS else SUPPORTED_PROTOCOLS[-1]
            session = uuid.uuid4().hex
            with sessions_lock:
                sessions.add(session)
            return rpc_result(
                request_id,
                {
                    "protocolVersion": protocol,
                    "capabilities": {
                        "tools": {"listChanged": False},
                        "resources": {"subscribe": False, "listChanged": False},
                    },
                    "serverInfo": {"name": "bronzecast", "version": "0.1.0"},
                },
                headers={SESSION_HEADER: session},
            )
        if method == "ping":
            return rpc_result(request_id, {})
        if method == "tools/list":
            return rpc_result(request_id, {"tools": tools.TOOLS})
        if method == "tools/call":
            name = params.get("name")
            if not isinstance(name, str):
                return rpc_error(request_id, -32602, "Tool name is required")
            required_scope = (
                "bronzecast.write" if name in tools.WRITE_TOOLS else "bronzecast.read"
            )
            if not oauth.scope_allows(token_record, required_scope):
                return unauthorized(
                    "insufficient_scope",
                    f"{required_scope} is required for {name}",
                    required_scope,
                )
            arguments = params.get("arguments")
            if arguments is None:
                arguments = {}
            if not isinstance(arguments, dict):
                return rpc_error(request_id, -32602, "arguments must be an object")
            try:
                result = await tools.call_tool(name, arguments, settings)
            except tools.ToolError as error:
                result = {
                    "content": [{"type": "text", "text": str(error)}],
                    "isError": True,
                }
            except Exception as error:  # pragma: no cover - defensive
                LOG.exception("tool %s failed", name)
                result = {
                    "content": [{"type": "text", "text": f"Internal error: {error}"}],
                    "isError": True,
                }
            return rpc_result(request_id, result)
        if method == "resources/list":
            return rpc_result(
                request_id,
                {
                    "resources": [
                        {
                            "uri": "bronzecast://dashboard/state",
                            "name": "Dashboard state",
                            "description": "Character, stickers, notifications and stream health snapshot.",
                            "mimeType": "application/json",
                        }
                    ]
                },
            )
        if method == "resources/read":
            uri = params.get("uri")
            if uri != "bronzecast://dashboard/state":
                return rpc_error(request_id, -32602, f"Unknown resource: {uri}")
            payload_state, _ = await tools._status_payload(settings)
            return rpc_result(
                request_id,
                {
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": json.dumps(payload_state, indent=2, sort_keys=True),
                        }
                    ]
                },
            )
        if method and str(method).startswith("notifications/"):
            return Response(status_code=202)
        if request_id is None:
            return Response(status_code=202)
        return rpc_error(request_id, -32601, f"Method not found: {method}")

    async def health(request):
        return JSONResponse(
            {
                "status": "ok",
                "service": "bronzecast-mcp",
                "issuer": settings.issuer,
                "keys_configured": len(settings.access_keys),
            }
        )

    async def landing(request):
        return PlainTextResponse(
            "BRONZECAST MCP\n\n"
            f"MCP endpoint: {settings.resource}\n"
            f"Authorization metadata: {settings.issuer}/.well-known/oauth-authorization-server\n"
            f"Protected resource metadata: {settings.issuer}/.well-known/oauth-protected-resource\n"
        )

    routes = [
        Route("/", landing),
        Route("/health", health),
        Route("/.well-known/oauth-authorization-server", metadata),
        Route("/.well-known/oauth-protected-resource", resource_metadata),
        Route("/.well-known/oauth-protected-resource/mcp", resource_metadata),
        Route("/register", register, methods=["POST"]),
        Route("/authorize", authorize_get),
        Route("/authorize", authorize_post, methods=["POST"]),
        Route("/token", token, methods=["POST"]),
        Route("/revoke", revoke, methods=["POST"]),
        Route("/mcp", mcp, methods=["POST", "GET", "DELETE"]),
    ]
    app = Starlette(routes=routes)
    app.state.settings = settings
    app.state.store = store
    app.state.sessions = sessions
    return app


def main():
    logging.basicConfig(
        level=os.getenv("LOG_LEVEL", "INFO"),
        format="%(asctime)s %(levelname)s %(message)s",
    )
    port = int(os.getenv("PORT", "8090"))
    uvicorn.run(
        "mcp.server:create_app",
        factory=True,
        host="0.0.0.0",
        port=port,
        log_level=os.getenv("LOG_LEVEL", "info").lower(),
    )


if __name__ == "__main__":
    main()
