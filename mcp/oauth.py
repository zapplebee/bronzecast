"""OAuth 2.1 authorization server logic for the BRONZECAST MCP endpoint.

Supports public clients registered through Dynamic Client Registration and
Client ID Metadata Documents, PKCE (S256 only), static-key login, opaque
access/refresh tokens, and RFC 8707 resource binding.
"""
import base64
import hashlib
import hmac
import re
import secrets
import time
from dataclasses import dataclass
from urllib.parse import urlsplit, urlencode, parse_qs, urlunsplit

from . import cimd

SUPPORTED_SCOPES = ("bronzecast.read", "bronzecast.write", "offline_access")
DEFAULT_SCOPE = "bronzecast.read bronzecast.write offline_access"
CHALLENGE_RE = re.compile(r"^[A-Za-z0-9\-._~]{43,128}$")
VERIFIER_RE = CHALLENGE_RE


class OAuthError(Exception):
    def __init__(self, error, description, status=400):
        super().__init__(description)
        self.error = error
        self.description = description
        self.status = status


class ClientError(OAuthError):
    """The client or redirect URI is unusable, so no redirect is allowed."""


def authorization_server_metadata(settings):
    return {
        "issuer": settings.issuer,
        "authorization_endpoint": settings.issuer + "/authorize",
        "token_endpoint": settings.issuer + "/token",
        "registration_endpoint": settings.issuer + "/register",
        "revocation_endpoint": settings.issuer + "/revoke",
        "scopes_supported": list(SUPPORTED_SCOPES),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"],
        "client_id_metadata_document_supported": True,
        "authorization_response_iss_parameter_supported": True,
    }


def protected_resource_metadata(settings):
    return {
        "resource": settings.resource,
        "authorization_servers": [settings.issuer],
        "scopes_supported": list(SUPPORTED_SCOPES),
        "bearer_methods_supported": ["header"],
    }


def valid_redirect_uri(uri):
    if not isinstance(uri, str) or len(uri) > 2048:
        return False
    parsed = urlsplit(uri)
    if parsed.fragment or parsed.username or parsed.password or not parsed.hostname:
        return False
    if parsed.scheme == "https":
        return True
    if parsed.scheme == "http":
        return parsed.hostname in ("127.0.0.1", "::1", "localhost")
    return False


def normalize_redirect_uris(value):
    if not isinstance(value, list) or not value:
        raise OAuthError("invalid_client_metadata", "redirect_uris is required")
    if len(value) > 10:
        raise OAuthError("invalid_client_metadata", "at most 10 redirect_uris are allowed")
    cleaned = []
    for uri in value:
        if not valid_redirect_uri(uri):
            raise OAuthError(
                "invalid_client_metadata",
                f"redirect_uri must use https or loopback http: {uri!r}",
            )
        if uri in cleaned:
            raise OAuthError("invalid_client_metadata", "redirect_uris must be unique")
        cleaned.append(uri)
    return cleaned


def normalize_client(document, client_id):
    if not isinstance(document, dict):
        raise OAuthError("invalid_client_metadata", "client metadata must be an object")
    if document.get("token_endpoint_auth_method", "none") != "none":
        raise OAuthError(
            "invalid_client_metadata",
            "only public clients with token_endpoint_auth_method=none are supported",
        )
    grant_types = document.get("grant_types", ["authorization_code"])
    if (
        not isinstance(grant_types, list)
        or "authorization_code" not in grant_types
        or not set(grant_types) <= {"authorization_code", "refresh_token"}
    ):
        raise OAuthError(
            "invalid_client_metadata",
            "grant_types must include authorization_code and only known grants",
        )
    response_types = document.get("response_types", ["code"])
    if response_types != ["code"]:
        raise OAuthError(
            "invalid_client_metadata", "response_types must be ['code']"
        )
    redirect_uris = normalize_redirect_uris(document.get("redirect_uris"))
    name = document.get("client_name")
    if not isinstance(name, str) or not name.strip():
        name = urlsplit(redirect_uris[0]).hostname or "MCP client"
    return {
        "client_id": client_id,
        "client_name": name.strip()[:120],
        "redirect_uris": redirect_uris,
        "grant_types": list(grant_types),
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    }


def register_client(store, document):
    client_id = "bc_" + secrets.token_urlsafe(18)
    metadata = normalize_client(document, client_id)
    store.put_client(client_id, metadata, "dcr")
    return {**metadata, "client_id_issued_at": int(time.time())}


def resolve_client(store, client_id, fetcher=None):
    if not isinstance(client_id, str) or not client_id:
        raise ClientError("invalid_client", "client_id is required")
    if client_id.startswith("https://"):
        try:
            document = cimd.fetch_client_metadata(client_id, fetcher)
        except cimd.ClientMetadataError as error:
            raise ClientError("invalid_client", str(error)) from error
        return normalize_client(document, client_id)
    stored = store.get_client(client_id)
    if stored is None:
        raise ClientError(
            "invalid_client", "unknown client_id; register dynamically or use a CIMD URL"
        )
    return stored


def key_matches(settings, supplied):
    if not settings.access_keys:
        return False
    return any(
        hmac.compare_digest((supplied or "").encode(), key.encode())
        for key in settings.access_keys
    )


@dataclass(frozen=True)
class AuthorizeContext:
    client_id: str
    client_name: str
    redirect_uri: str
    state: str
    scope: str
    resource: str
    challenge: str


def build_authorize_context(params, client, settings):
    # Validate the redirect URI before any redirectable error can be produced.
    redirect_uri = params.get("redirect_uri") or ""
    if redirect_uri not in client["redirect_uris"]:
        raise ClientError(
            "invalid_request", "redirect_uri does not exactly match a registered value"
        )
    if params.get("response_type") != "code":
        raise OAuthError(
            "unsupported_response_type", "only response_type=code is supported"
        )
    state = params.get("state") or ""
    if len(state) > 1024:
        raise OAuthError("invalid_request", "state is too long")
    challenge = params.get("code_challenge") or ""
    if not CHALLENGE_RE.fullmatch(challenge):
        raise OAuthError(
            "invalid_request", "code_challenge must be a 43-128 character PKCE value"
        )
    if params.get("code_challenge_method") != "S256":
        raise OAuthError("invalid_request", "code_challenge_method must be S256")
    requested = []
    for scope in (params.get("scope") or DEFAULT_SCOPE).split():
        if scope not in SUPPORTED_SCOPES:
            raise OAuthError("invalid_scope", f"unsupported scope {scope!r}")
        if scope not in requested:
            requested.append(scope)
    if not {"bronzecast.read", "bronzecast.write"} & set(requested):
        raise OAuthError("invalid_scope", "a bronzecast scope is required")
    resource = params.get("resource") or settings.resource
    if resource != settings.resource:
        raise OAuthError(
            "invalid_target", "resource must identify this MCP endpoint"
        )
    return AuthorizeContext(
        client_id=client["client_id"],
        client_name=client["client_name"],
        redirect_uri=redirect_uri,
        state=state,
        scope=" ".join(requested),
        resource=resource,
        challenge=challenge,
    )


def verify_pkce(verifier, challenge):
    if not VERIFIER_RE.fullmatch(verifier or ""):
        return False
    digest = hashlib.sha256(verifier.encode("ascii")).digest()
    computed = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    return hmac.compare_digest(computed, challenge)


def redirect_url(redirect_uri, values):
    parts = urlsplit(redirect_uri)
    query = parse_qs(parts.query, keep_blank_values=True)
    for key, value in values.items():
        if value is not None:
            query[key] = [value]
    return urlunsplit(
        (parts.scheme, parts.netloc, parts.path, urlencode(query, doseq=True), "")
    )


def issue_tokens(settings, store, client_id, scope, resource, family=None):
    family = family or secrets.token_hex(16)
    access = store.create_token(
        "access", client_id, scope, resource, family, settings.access_ttl
    )
    body = {
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": settings.access_ttl,
        "scope": scope,
    }
    if "offline_access" in scope.split():
        body["refresh_token"] = store.create_token(
            "refresh", client_id, scope, resource, family, settings.refresh_ttl
        )
    return body


def exchange_token_request(settings, store, form):
    grant = form.get("grant_type")
    if grant == "authorization_code":
        return _authorization_code(settings, store, form)
    if grant == "refresh_token":
        return _refresh_token(settings, store, form)
    raise OAuthError(
        "unsupported_grant_type", "grant_type must be authorization_code or refresh_token"
    )


def _authorization_code(settings, store, form):
    code = form.get("code")
    client_id = form.get("client_id")
    redirect_uri = form.get("redirect_uri")
    verifier = form.get("code_verifier")
    if not code or not client_id or not redirect_uri or not verifier:
        raise OAuthError(
            "invalid_request",
            "code, client_id, redirect_uri and code_verifier are required",
        )
    row = store.take_code(code)
    if row is None:
        raise OAuthError("invalid_grant", "authorization code is unknown or expired")
    if row["client_id"] != client_id or row["redirect_uri"] != redirect_uri:
        raise OAuthError("invalid_grant", "authorization code does not match this client")
    if not verify_pkce(verifier, row["challenge"]):
        raise OAuthError("invalid_grant", "PKCE verification failed")
    resource = form.get("resource")
    if resource and resource != row["resource"]:
        raise OAuthError("invalid_target", "resource does not match the authorization")
    return issue_tokens(
        settings, store, client_id, row["scope"], row["resource"]
    )


def _refresh_token(settings, store, form):
    raw = form.get("refresh_token")
    client_id = form.get("client_id")
    if not raw or not client_id:
        raise OAuthError(
            "invalid_request", "refresh_token and client_id are required"
        )
    row, replacement = store.rotate_refresh(raw, client_id, settings.refresh_ttl)
    if row is None:
        raise OAuthError("invalid_grant", "refresh token is invalid or was already used")
    resource = form.get("resource")
    if resource and resource != row["resource"]:
        raise OAuthError("invalid_target", "resource does not match the authorization")
    body = issue_tokens(
        settings, store, client_id, row["scope"], row["resource"], row["family"]
    )
    body["refresh_token"] = replacement
    return body


def token_from_request(settings, store, authorization):
    if not authorization or not authorization.lower().startswith("bearer "):
        return None
    raw = authorization[7:].strip()
    if not raw:
        return None
    row = store.find_token(raw, "access")
    if row is None or row["revoked"] or row["expires_at"] <= int(time.time()):
        return None
    if row["resource"] != settings.resource:
        return None
    return row


def scope_allows(token, scope):
    return scope in (token.get("scope") or "").split()
