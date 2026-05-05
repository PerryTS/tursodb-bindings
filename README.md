# @perryts/tursodb

Native bindings for [Tursodb](https://github.com/tursodatabase/turso) — a pure-Rust SQLite-compatible engine — for the [Perry TypeScript-to-native compiler](https://github.com/PerryTS/perry).

Closes [PerryTS/perry#424](https://github.com/PerryTS/perry/issues/424).

## What this is

A Perry "native library" package: a Rust crate exporting `extern "C"` symbols that the Perry compiler links into your TypeScript program. From your TypeScript code you import `tursodb` like any npm package; under the hood every method call resolves to a direct call into the bundled staticlib.

## Install

```sh
bun add @perryts/tursodb
# or
npm install @perryts/tursodb
```

The package's `package.json` declares a `perry.nativeLibrary` block (see the [manifest spec](https://github.com/PerryTS/perry/blob/main/docs/src/native-libraries/manifest-v1.md)) which Perry's compiler reads at link time to discover the staticlib + `extern "C"` symbols.

## Usage

```typescript
import * as tursodb from "tursodb";

const db = await tursodb.open(":memory:");

await tursodb.execBatch(db, `
  CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT);
  INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
  INSERT INTO users (name, email) VALUES ('Bob',   'bob@example.com');
`);

// queryAll returns an array of row-objects keyed by column name.
const rows = await tursodb.queryAll(db, "SELECT * FROM users ORDER BY id");
for (const row of rows) {
  console.log(row.id, row.name, row.email);
}

// queryOne returns the first row, or null.
const alice = await tursodb.queryOne(db, "SELECT * FROM users WHERE name = 'Alice'");
console.log(alice?.email);

const newRowsAffected = await tursodb.exec(db, "DELETE FROM users WHERE id = 2");
console.log("deleted rows:", newRowsAffected);

await tursodb.close(db);
```

## API

| Function | Type | Notes |
|---|---|---|
| `open(path)` | `Promise<handle>` | `":memory:"` for an in-memory database, otherwise a filesystem path |
| `exec(handle, sql)` | `Promise<number>` | Returns rows-affected for a non-query statement |
| `execBatch(handle, sql)` | `Promise<void>` | Multiple `;`-separated statements |
| `queryAll(handle, sql)` | `Promise<Array<Object>>` | Each row is an object keyed by column name |
| `queryOne(handle, sql)` | `Promise<Object \| null>` | First row or null |
| `lastInsertRowid(handle)` | `number` | Sync accessor for the last `INSERT`'s row id |
| `isAutocommit(handle)` | `boolean` | Sync |
| `close(handle)` | `boolean` | Drops the connection |

## Status

MVP — `prepare()` + parameter binding (`?` placeholders) is the next gap. Tracked in the upstream [`PerryTS/perry`](https://github.com/PerryTS/perry) repo.

## License

MIT — see [LICENSE](./LICENSE).
