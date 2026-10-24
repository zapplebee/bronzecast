"""SQLite persistence for OAuth clients, authorization codes and tokens.

Only hashes of codes and tokens are stored. A single connection guarded by a
lock is sufficient for the playout-scale traffic this service serves.
"""
import hashlib
import json
import pathlib
import secrets
import sqlite3
import threading
import time

SCHEMA = """
CREATE TABLE IF NOT EXISTS clients (
  client_id TEXT PRIMARY KEY,
  metadata TEXT NOT NULL,
  source TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS codes (
  code_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  redirect_uri TEXT NOT NULL,
  scope TEXT NOT NULL,
  resource TEXT NOT NULL,
  challenge TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tokens (
  token_hash TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  client_id TEXT NOT NULL,
  scope TEXT NOT NULL,
  resource TEXT NOT NULL,
  family TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS tokens_family ON tokens(family);
"""


def token_hash(value):
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


class Store:
    def __init__(self, path):
        if path != ":memory:":
            pathlib.Path(path).parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.RLock()
        self._db = sqlite3.connect(path, check_same_thread=False)
        self._db.row_factory = sqlite3.Row
        self._db.execute("PRAGMA journal_mode=WAL")
        self._db.executescript(SCHEMA)
        self._db.commit()

    def close(self):
        with self._lock:
            self._db.close()

    def put_client(self, client_id, metadata, source):
        with self._lock:
            self._db.execute(
                "INSERT OR REPLACE INTO clients(client_id, metadata, source, created_at)"
                " VALUES (?, ?, ?, ?)",
                (client_id, json.dumps(metadata), source, int(time.time())),
            )
            self._db.commit()
        return metadata

    def get_client(self, client_id):
        with self._lock:
            row = self._db.execute(
                "SELECT metadata FROM clients WHERE client_id = ?", (client_id,)
            ).fetchone()
        return json.loads(row["metadata"]) if row else None

    def client_count(self):
        with self._lock:
            return self._db.execute("SELECT COUNT(*) FROM clients").fetchone()[0]

    def create_code(self, client_id, redirect_uri, scope, resource, challenge, ttl):
        raw = secrets.token_urlsafe(32)
        now = int(time.time())
        with self._lock:
            self._db.execute(
                "INSERT INTO codes(code_hash, client_id, redirect_uri, scope, resource,"
                " challenge, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                (token_hash(raw), client_id, redirect_uri, scope, resource, challenge, now, now + ttl),
            )
            self._db.commit()
        return raw

    def take_code(self, raw):
        digest = token_hash(raw)
        now = int(time.time())
        with self._lock:
            row = self._db.execute(
                "SELECT * FROM codes WHERE code_hash = ?", (digest,)
            ).fetchone()
            self._db.execute("DELETE FROM codes WHERE code_hash = ?", (digest,))
            self._db.commit()
        if row is None or row["expires_at"] <= now:
            return None
        return dict(row)

    def create_token(self, kind, client_id, scope, resource, family, ttl):
        raw = secrets.token_urlsafe(32)
        now = int(time.time())
        with self._lock:
            self._db.execute(
                "INSERT INTO tokens(token_hash, kind, client_id, scope, resource, family,"
                " created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                (token_hash(raw), kind, client_id, scope, resource, family, now, now + ttl),
            )
            self._db.commit()
        return raw

    def find_token(self, raw, kind):
        with self._lock:
            row = self._db.execute(
                "SELECT * FROM tokens WHERE token_hash = ? AND kind = ?",
                (token_hash(raw), kind),
            ).fetchone()
        return dict(row) if row else None

    def rotate_refresh(self, raw, client_id, ttl):
        """Consume a refresh token and issue its replacement.

        Reuse of an already rotated token revokes the whole family.
        """
        digest = token_hash(raw)
        now = int(time.time())
        with self._lock:
            row = self._db.execute(
                "SELECT * FROM tokens WHERE token_hash = ? AND kind = 'refresh'", (digest,)
            ).fetchone()
            if row is None:
                return None, None
            if row["revoked"]:
                self._db.execute(
                    "UPDATE tokens SET revoked = 1 WHERE family = ?", (row["family"],)
                )
                self._db.commit()
                return None, None
            if row["expires_at"] <= now or row["client_id"] != client_id:
                return None, None
            self._db.execute(
                "UPDATE tokens SET revoked = 1 WHERE token_hash = ?", (digest,)
            )
            self._db.commit()
            replacement = self.create_token(
                "refresh",
                row["client_id"],
                row["scope"],
                row["resource"],
                row["family"],
                ttl,
            )
        return dict(row), replacement

    def revoke(self, raw):
        with self._lock:
            self._db.execute(
                "UPDATE tokens SET revoked = 1 WHERE token_hash = ?", (token_hash(raw),)
            )
            self._db.commit()

    def prune(self, now=None):
        now = int(time.time()) if now is None else now
        with self._lock:
            self._db.execute("DELETE FROM codes WHERE expires_at <= ?", (now,))
            self._db.execute(
                "DELETE FROM tokens WHERE kind = 'access' AND expires_at <= ?", (now,)
            )
            self._db.commit()
