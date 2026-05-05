declare module "tursodb" {
  export type Database = number & { readonly __tursodb: unique symbol };

  export type Value = string | number | null;
  export type Row = Record<string, Value>;

  export function open(path: string): Promise<Database>;
  export function exec(db: Database, sql: string): Promise<number>;
  export function execBatch(db: Database, sql: string): Promise<void>;
  export function queryAll(db: Database, sql: string): Promise<Row[]>;
  export function queryOne(db: Database, sql: string): Promise<Row | null>;
  export function lastInsertRowid(db: Database): number;
  export function isAutocommit(db: Database): boolean;
  export function close(db: Database): boolean;
}

declare module "@perryts/tursodb" {
  export * from "tursodb";
}
