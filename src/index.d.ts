declare module "tursodb" {
  export type Database = number & { readonly __tursodb: unique symbol };

  export type Value = string | number | null;
  export type Row = Record<string, Value>;

  // A SQL parameter — primitives or a Uint8-byte array (→ SQLite Blob).
  // Nested objects are rejected at runtime; pre-stringify if needed.
  export type Param = string | number | boolean | null | number[];

  export function open(path: string): Promise<Database>;

  export function exec(db: Database, sql: string): Promise<number>;
  export function execBatch(db: Database, sql: string): Promise<void>;
  export function queryAll(db: Database, sql: string): Promise<Row[]>;
  export function queryOne(db: Database, sql: string): Promise<Row | null>;

  // Parameterized variants (v0.2.0) — bind `?` placeholders from the
  // params array. Use these when SQL values come from untrusted
  // input; the unparameterized variants are for static SQL only.
  export function execWith(db: Database, sql: string, params: Param[]): Promise<number>;
  export function queryAllWith(db: Database, sql: string, params: Param[]): Promise<Row[]>;
  export function queryOneWith(db: Database, sql: string, params: Param[]): Promise<Row | null>;

  export function lastInsertRowid(db: Database): number;
  export function isAutocommit(db: Database): boolean;
  export function close(db: Database): boolean;
}

declare module "@perryts/tursodb" {
  export * from "tursodb";
}
