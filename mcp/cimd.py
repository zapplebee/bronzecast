"""Client ID Metadata Documents (CIMD) fetching and validation.

The client_id itself is an HTTPS URL pointing at a JSON document that mirrors
OAuth client metadata. Fetching must not become an SSRF primitive, so only
public addresses are contacted, redirects are refused, and the response size
is bounded.
"""
import ipaddress
import json
import socket
import threading
import time
from urllib.parse import urlsplit

import httpx

MAX_DOCUMENT_BYTES = 64 * 1024
FETCH_TIMEOUT = 5.0
DEFAULT_TTL = 300
MAX_TTL = 900
MIN_TTL = 60

_CACHE = {}
_CACHE_LOCK = threading.Lock()


class ClientMetadataError(ValueError):
    pass


def validate_client_id(client_id):
    parsed = urlsplit(client_id)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.fragment
        or parsed.username
        or parsed.password
    ):
        raise ClientMetadataError(
            "client_id must be an absolute https URL without credentials or fragment"
        )
    return parsed


def ensure_public_host(hostname):
    try:
        addresses = {
            item[4][0].split("%")[0]
            for item in socket.getaddrinfo(hostname, 443, type=socket.SOCK_STREAM)
        }
    except socket.gaierror as error:
        raise ClientMetadataError(
            f"could not resolve client_id host: {error}"
        ) from error
    if not addresses:
        raise ClientMetadataError("client_id host resolved to no addresses")
    for value in addresses:
        try:
            address = ipaddress.ip_address(value)
        except ValueError as error:
            raise ClientMetadataError(
                f"client_id host resolved to an invalid address: {value}"
            ) from error
        if not address.is_global:
            raise ClientMetadataError(
                "client_id host resolves to a non-public address"
            )


def _fetch(url):
    ensure_public_host(urlsplit(url).hostname)
    try:
        with httpx.Client(follow_redirects=False, timeout=FETCH_TIMEOUT) as client:
            with client.stream(
                "GET", url, headers={"Accept": "application/json"}
            ) as response:
                return _read_response(response)
    except httpx.HTTPError as error:
        raise ClientMetadataError(
            f"could not fetch client_id document: {error}"
        ) from error


def _read_response(response):
    if response.status_code != 200:
        raise ClientMetadataError(
            f"client_id document returned HTTP {response.status_code}"
        )
    media_type = (
        response.headers.get("content-type", "").split(";")[0].strip().lower()
    )
    if media_type != "application/json":
        raise ClientMetadataError(
            "client_id document must be served as application/json"
        )
    total = 0
    chunks = []
    for chunk in response.iter_bytes():
        total += len(chunk)
        if total > MAX_DOCUMENT_BYTES:
            raise ClientMetadataError("client_id document is too large")
        chunks.append(chunk)
    return dict(response.headers), b"".join(chunks)


def _cache_ttl(headers):
    for part in (headers or {}).get("cache-control", "").split(","):
        name, _, value = part.strip().partition("=")
        if name.lower() == "max-age" and value.strip().isdigit():
            return max(MIN_TTL, min(MAX_TTL, int(value)))
    return DEFAULT_TTL


def clear_cache():
    with _CACHE_LOCK:
        _CACHE.clear()


def fetch_client_metadata(client_id, fetcher=None, now=None):
    """Fetch and minimally validate a CIMD document.

    ``fetcher`` is injectable for tests; when supplied it bypasses networking
    (and therefore the DNS guard), so callers must only use it in tests.
    """
    now = time.monotonic() if now is None else now
    with _CACHE_LOCK:
        cached = _CACHE.get(client_id)
        if cached and cached[0] > now:
            return dict(cached[1])

    validate_client_id(client_id)
    headers, body = (fetcher or _fetch)(client_id)
    try:
        document = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ClientMetadataError("client_id document is not valid JSON") from error
    if not isinstance(document, dict):
        raise ClientMetadataError("client_id document must be a JSON object")
    if document.get("client_id") != client_id:
        raise ClientMetadataError(
            "client_id document must repeat the exact client_id URL"
        )

    with _CACHE_LOCK:
        _CACHE[client_id] = (now + _cache_ttl(headers), document)
    return dict(document)
