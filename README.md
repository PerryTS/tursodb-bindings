# @perryts/tursodb

Native bindings for [Tursodb](https://github.com/tursodatabase/turso) — a pure-Rust SQLite-compatible engine — for the [Perry TypeScript-to-native compiler](https://github.com/PerryTS/perry).

Closes [PerryTS/perry#424](https://github.com/PerryTS/perry/issues/424).

## What this is

A Perry "native library" package: a Rust crate exporting `extern "C"` symbols that the Perry compiler links into your TypeScript program. From your TypeScript code you import `tursodb` like any npm package; under the hood every method call resolves to a direct call into the bundled staticlib — no Node addon, no IPC, no JSON marshalling.

This package contains:

- `src/lib.rs` — the Rust crate that wraps `turso` and exposes `js_turso_*` `extern "C"` symbols
- `src/index.d.ts` — the TypeScript surface (`tursodb` module declaration) Perry resolves at compile time
- `Cargo.toml` — staticlib build config consumed by the Perry linker
- `package.json` — includes the `perry.nativeLibrary` manifest block

## Install

```sh
bun add @perryts/tursodb
# or
npm install @perryts/tursodb
```

The package's `package.json` declares a `perry.nativeLibrary` block (see the [manifest spec](https://github.com/PerryTS/perry/blob/main/docs/src/native-libraries/manifest-v1.md)) which Perry's compiler reads at link time to discover the staticlib + `extern "C"` symbols. No post-install build step — Perry compiles the Rust crate as part of your project's build.

## Quick start

```typescript
import * as tursodb from "tursodb";

const db = await tursodb.open(":memory:");

await tursodb.execBatch(db, `
  CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT);
  INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
  INSERT INTO users (name, email) VALUES ('Bob',   'bob@example.com');
`);

const rows = await tursodb.queryAll(db, "SELECT * FROM users ORDER BY id");
for (const row of rows) {
  console.log(row.id, row.name, row.email);
}

await tursodb.close(db);
```

## API reference

### `open(path)`

```typescript
function open(path: string): Promise<Database>
```

Open a database connection. Use `":memory:"` for an in-memory database, otherwise pass a filesystem path (created if missing). Resolves with an opaque `Database` handle. Reject reasons surface the underlying turso error.

```typescript
const memory = await tursodb.open(":memory:");
const onDisk  = await tursodb.open("./app.db");
```

### `exec(db, sql)`

```typescript
function exec(db: Database, sql: string): Promise<number>
```

Execute a single non-query statement. Resolves with the number of rows affected. Use this for `INSERT` / `UPDATE` / `DELETE` and DDL.

```typescript
const inserted = await tursodb.exec(db, "INSERT INTO users (name) VALUES ('Carol')");
const removed  = await tursodb.exec(db, "DELETE FROM users WHERE id = 2");
```

### `execBatch(db, sql)`

```typescript
function execBatch(db: Database, sql: string): Promise<void>
```

Execute multiple `;`-separated statements in one round-trip. Useful for migrations, schema bootstrap, and seed scripts. Does not return row counts.

```typescript
await tursodb.execBatch(db, `
  CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT, body TEXT);
  CREATE INDEX idx_posts_title ON posts(title);
`);
```

### `queryAll(db, sql)`

```typescript
function queryAll(db: Database, sql: string): Promise<Row[]>
```

Run a `SELECT` and resolve with every row as an object keyed by column name. Equivalent to `better-sqlite3`'s `stmt.all()`.

```typescript
const rows = await tursodb.queryAll(db, "SELECT id, name FROM users ORDER BY id");
// rows: [{ id: 1, name: "Alice" }, { id: 3, name: "Carol" }]
```

### `queryOne(db, sql)`

```typescript
function queryOne(db: Database, sql: string): Promise<Row | null>
```

Run a query and resolve with the first row, or `null` if the result set is empty. Equivalent to `better-sqlite3`'s `stmt.get()`.

```typescript
const alice = await tursodb.queryOne(db, "SELECT * FROM users WHERE name = 'Alice'");
console.log(alice?.email);
```

### `lastInsertRowid(db)`

```typescript
function lastInsertRowid(db: Database): number
```

Synchronous accessor for the rowid of the most recent successful `INSERT` on this connection. Returns `0` if no insert has happened yet.

```typescript
await tursodb.exec(db, "INSERT INTO users (name) VALUES ('Dan')");
const newId = tursodb.lastInsertRowid(db);
```

### `isAutocommit(db)`

```typescript
function isAutocommit(db: Database): boolean
```

Synchronous. Returns `true` when the connection is outside an explicit transaction, `false` while a `BEGIN` block is open.

```typescript
await tursodb.exec(db, "BEGIN");
tursodb.isAutocommit(db); // false
await tursodb.exec(db, "COMMIT");
tursodb.isAutocommit(db); // true
```

### `close(db)`

```typescript
function close(db: Database): boolean
```

Synchronous. Drops the connection and frees the underlying handle. Returns `true` on success, `false` if the handle was already closed or invalid. Calling any other function on a closed handle rejects with `"tursodb: invalid handle"`.

```typescript
tursodb.close(db);
```

## Types

Exported from the `tursodb` module declaration in `src/index.d.ts`:

```typescript
type Database = number & { readonly __tursodb: unique symbol };
type Value    = string | number | null;
type Row      = Record<string, Value>;
```

Column-value mapping (from the Rust `turso_value_to_js`):

| SQLite type | TypeScript value |
|---|---|
| `NULL` | `null` |
| `INTEGER` | `number` |
| `REAL` | `number` |
| `TEXT` | `string` |
| `BLOB` | `string` (lowercase hex) |

`BLOB` columns are rendered as a lowercase hex string to keep the wrapper dependency-free; decode them in user code if you need raw bytes.

## Transactions

Use `BEGIN` / `COMMIT` / `ROLLBACK` via `exec`. There is no high-level transaction wrapper yet.

```typescript
await tursodb.exec(db, "BEGIN");
try {
  await tursodb.exec(db, "INSERT INTO users (name) VALUES ('Eve')");
  await tursodb.exec(db, "INSERT INTO users (name) VALUES ('Frank')");
  await tursodb.exec(db, "COMMIT");
} catch (err) {
  await tursodb.exec(db, "ROLLBACK");
  throw err;
}
```

## Error handling

Async functions reject with an `Error` whose message is prefixed by the operation, e.g. `tursodb queryAll: <upstream message>`. An invalid or already-closed handle rejects with `"tursodb: invalid handle"`. Sync functions (`lastInsertRowid`, `isAutocommit`, `close`) do not throw — `lastInsertRowid` returns `0` on a bad handle, `isAutocommit` returns `false`, `close` returns `false`.

## Status & roadmap

MVP. What's there:

- `open` / `close`
- `exec` / `execBatch`
- `queryAll` / `queryOne` (rows materialized as objects keyed by column name)
- `lastInsertRowid` / `isAutocommit`

Known gaps, tracked in [`PerryTS/perry`](https://github.com/PerryTS/perry):

- Parameter binding (`?` placeholders) — needs a JS `Array<JsValue>` reader on the perry-ffi side that maps onto turso's `IntoParams`. Until then, embed values directly in SQL — but only for trusted input.
- Prepared-statement objects (`stmt.all()` / `stmt.get()` / `stmt.run()` style)
- Streaming / iterator-based row reads
- Native `BLOB` (`Uint8Array`) values instead of hex strings
- High-level transaction helper

## Versioning

Pre-1.0. The `perry.nativeLibrary.abiVersion` (currently `0.5`) is a hard pin against Perry's perry-ffi ABI — bump it in lockstep with the Perry release that the bindings target.

## License

MIT — see [LICENSE](./LICENSE).
