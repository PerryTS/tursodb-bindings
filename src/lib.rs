//! Native bindings for Tursodb (closes #424).
//!
//! [Tursodb](https://github.com/tursodatabase) is a pure-Rust
//! SQLite-compatible database engine with extras (MVCC,
//! encryption, custom VFS). The wrapper exposes a TypeScript
//! surface modeled on `better-sqlite3`'s async equivalents
//! (since turso's API is async-first).
//!
//! # Status
//!
//! - v0.5.543: open / exec / execBatch / close / lastInsertRowid /
//!   isAutocommit (the original MVP cut from #424).
//! - v0.5.553: queryAll / queryOne (rows-as-objects via
//!   `build_object_shape` + `js_object_alloc_with_shape` +
//!   `js_array_alloc` / `js_array_push`). Closes the row-shape
//!   construction gap that the v0.5.543 docstring mentioned as a
//!   followup. Param-binding (`?` placeholders) is the next gap —
//!   needs a JS `Array<JsValue>` reader on the perry-ffi side that
//!   maps onto turso's `IntoParams`.
//!
//! # Recipe
//!
//! Same as every other async wrapper: register a handle holding
//! the `turso::Connection`; method calls take the handle plus
//! params, spawn blocking onto perry-ffi's tokio runtime, and
//! resolve a `JsPromise` from inside the closure. `tokio::runtime
//! ::Handle::current().block_on(async { ... })` bridges turso's
//! async API to the synchronous `spawn_blocking` closure body.

use perry_ffi::{
    alloc_string, build_object_shape, drop_handle, get_handle, js_array_alloc, js_array_push,
    js_object_alloc_with_shape, js_object_set_field, read_string, register_handle, spawn_blocking,
    with_handle, Handle, JsPromise, JsString, JsValue, Promise, StringHeader,
};
use turso::{Builder, Connection};

/// Wrapper struct so the registry's downcast is uniquely typed.
pub struct TursoConn {
    pub conn: Connection,
}

unsafe fn read_str(ptr: *const StringHeader) -> Option<String> {
    let handle = JsString::from_raw(ptr as *mut StringHeader);
    read_string(handle).map(String::from)
}

/// `tursodb.open(filename) -> Promise<Handle>` — open a database
/// and return a connection handle. `:memory:` for an in-memory
/// database, otherwise a filesystem path.
///
/// # Safety
///
/// `filename_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_turso_open(filename_ptr: *const StringHeader) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let path = read_str(filename_ptr).unwrap_or_else(|| ":memory:".to_string());

    spawn_blocking(move || {
        let result = tokio::runtime::Handle::current().block_on(async move {
            let db = Builder::new_local(&path).build().await?;
            let conn = db.connect()?;
            Ok::<Connection, turso::Error>(conn)
        });
        match result {
            Ok(conn) => {
                let handle = register_handle(TursoConn { conn });
                // Handles are i64; ABI for FFI booleans is f64 too,
                // so we encode the handle as a number value. The
                // TS-side wrapper unboxes and stores it.
                promise.resolve(JsValue::from_number(handle as f64));
            }
            Err(e) => promise.reject_string(&format!("tursodb open: {}", e)),
        }
    });
    raw
}

/// `tursodb.exec(handle, sql) -> Promise<number>` — execute a
/// non-query statement (or batch). Resolves with rows affected.
///
/// # Safety
///
/// `sql_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_turso_exec(
    db_handle: Handle,
    sql_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let Some(sql) = read_str(sql_ptr) else {
        promise.reject_string("Invalid SQL string");
        return raw;
    };

    spawn_blocking(move || {
        let result = with_handle::<TursoConn, _, _>(db_handle, |h| {
            tokio::runtime::Handle::current().block_on(async {
                h.conn.execute(&sql, ()).await
            })
        });
        match result {
            Some(Ok(rows_affected)) => {
                promise.resolve(JsValue::from_number(rows_affected as f64));
            }
            Some(Err(e)) => promise.reject_string(&format!("tursodb exec: {}", e)),
            None => promise.reject_string("tursodb: invalid handle"),
        }
    });
    raw
}

/// `tursodb.execBatch(handle, sql) -> Promise<void>` — execute
/// multiple statements separated by `;`.
///
/// # Safety
///
/// `sql_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_turso_exec_batch(
    db_handle: Handle,
    sql_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let Some(sql) = read_str(sql_ptr) else {
        promise.reject_string("Invalid SQL string");
        return raw;
    };

    spawn_blocking(move || {
        let result = with_handle::<TursoConn, _, _>(db_handle, |h| {
            tokio::runtime::Handle::current().block_on(async {
                h.conn.execute_batch(&sql).await
            })
        });
        match result {
            Some(Ok(())) => promise.resolve_undefined(),
            Some(Err(e)) => promise.reject_string(&format!("tursodb execBatch: {}", e)),
            None => promise.reject_string("tursodb: invalid handle"),
        }
    });
    raw
}

/// `tursodb.lastInsertRowid(handle) -> number` — synchronous
/// accessor for the last `INSERT`'s row id. The underlying
/// turso method is sync, no Promise wrapping needed.
#[no_mangle]
pub extern "C" fn js_turso_last_insert_rowid(db_handle: Handle) -> f64 {
    if let Some(h) = get_handle::<TursoConn>(db_handle) {
        h.conn.last_insert_rowid() as f64
    } else {
        0.0
    }
}

/// `tursodb.isAutocommit(handle) -> boolean (1.0 / 0.0)`.
#[no_mangle]
pub extern "C" fn js_turso_is_autocommit(db_handle: Handle) -> f64 {
    if let Some(h) = get_handle::<TursoConn>(db_handle) {
        match h.conn.is_autocommit() {
            Ok(true) => 1.0,
            _ => 0.0,
        }
    } else {
        0.0
    }
}

/// `tursodb.close(handle) -> 1.0 / 0.0` — drop the connection.
#[no_mangle]
pub extern "C" fn js_turso_close(db_handle: Handle) -> f64 {
    if drop_handle(db_handle) {
        1.0
    } else {
        0.0
    }
}

/// Convert a turso `Value` to a perry `JsValue`. Null/Integer/Real
/// fit inline; Text allocates a JS string; Blob is rendered as a
/// lowercase hex string (matches the perry-stdlib sqlite.rs
/// convention to avoid pulling the `hex` crate into the link).
fn turso_value_to_js(value: &turso::Value) -> JsValue {
    match value {
        turso::Value::Null => JsValue::NULL,
        turso::Value::Integer(n) => {
            if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                JsValue::from_int32(*n as i32)
            } else {
                JsValue::from_number(*n as f64)
            }
        }
        turso::Value::Real(n) => JsValue::from_number(*n),
        turso::Value::Text(s) => JsValue::from_string_ptr(alloc_string(s).as_raw()),
        turso::Value::Blob(b) => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut out = Vec::with_capacity(b.len() * 2);
            for &byte in b {
                out.push(HEX[(byte >> 4) as usize]);
                out.push(HEX[(byte & 0x0f) as usize]);
            }
            // SAFETY: HEX-table output is ASCII-only.
            let s = unsafe { std::str::from_utf8_unchecked(&out) };
            JsValue::from_string_ptr(alloc_string(s).as_raw())
        }
    }
}

/// Materialize a single `Row` into a JS object whose keys are the
/// statement's column names. Returns the NaN-boxed object pointer.
fn row_to_object(row: &turso::Row, column_names: &[String]) -> JsValue {
    let key_refs: Vec<&str> = column_names.iter().map(|s| s.as_str()).collect();
    let (packed_keys, shape_id) = build_object_shape(&key_refs);
    let field_count = column_names.len() as u32;
    let obj = unsafe {
        js_object_alloc_with_shape(
            shape_id,
            field_count,
            packed_keys.as_ptr(),
            packed_keys.len() as u32,
        )
    };
    for (i, _name) in column_names.iter().enumerate() {
        let val = row
            .get_value(i)
            .map(|v| turso_value_to_js(&v))
            .unwrap_or(JsValue::NULL);
        unsafe { js_object_set_field(obj, i as u32, val) };
    }
    JsValue::from_object_ptr(obj)
}

/// `tursodb.queryAll(handle, sql) -> Promise<Array<Object>>` — run a
/// query (no params) and resolve with every row as a row-object
/// keyed by column name. Brings the wrapper to the same level as
/// `better-sqlite3`'s `stmt.all()`.
///
/// # Safety
///
/// `sql_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_turso_query_all(
    db_handle: Handle,
    sql_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let Some(sql) = read_str(sql_ptr) else {
        promise.reject_string("Invalid SQL string");
        return raw;
    };

    spawn_blocking(move || {
        let outcome = with_handle::<TursoConn, _, _>(db_handle, |h| {
            tokio::runtime::Handle::current().block_on(async {
                let mut stmt = h.conn.prepare(&sql).await?;
                let column_names: Vec<String> =
                    stmt.columns().iter().map(|c| c.name().to_string()).collect();
                let mut rows = stmt.query(()).await?;
                let mut objects: Vec<JsValue> = Vec::new();
                while let Some(row) = rows.next().await? {
                    objects.push(row_to_object(&row, &column_names));
                }
                Ok::<Vec<JsValue>, turso::Error>(objects)
            })
        });
        match outcome {
            Some(Ok(objects)) => {
                let mut arr = js_array_alloc(objects.len() as u32);
                for obj in objects {
                    arr = js_array_push(arr, obj);
                }
                promise.resolve(JsValue::from_object_ptr(arr));
            }
            Some(Err(e)) => promise.reject_string(&format!("tursodb queryAll: {}", e)),
            None => promise.reject_string("tursodb: invalid handle"),
        }
    });
    raw
}

/// `tursodb.queryOne(handle, sql) -> Promise<Object | null>` — run a
/// query and resolve with the first row (or null if no rows).
///
/// # Safety
///
/// `sql_ptr` must be null or a Perry-runtime `StringHeader`.
#[no_mangle]
pub unsafe extern "C" fn js_turso_query_one(
    db_handle: Handle,
    sql_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = JsPromise::new();
    let raw = promise.as_raw();
    let Some(sql) = read_str(sql_ptr) else {
        promise.reject_string("Invalid SQL string");
        return raw;
    };

    spawn_blocking(move || {
        let outcome = with_handle::<TursoConn, _, _>(db_handle, |h| {
            tokio::runtime::Handle::current().block_on(async {
                let mut stmt = h.conn.prepare(&sql).await?;
                let column_names: Vec<String> =
                    stmt.columns().iter().map(|c| c.name().to_string()).collect();
                let mut rows = stmt.query(()).await?;
                let first = rows.next().await?;
                Ok::<Option<JsValue>, turso::Error>(
                    first.map(|row| row_to_object(&row, &column_names)),
                )
            })
        });
        match outcome {
            Some(Ok(Some(obj))) => promise.resolve(obj),
            Some(Ok(None)) => promise.resolve(JsValue::NULL),
            Some(Err(e)) => promise.reject_string(&format!("tursodb queryOne: {}", e)),
            None => promise.reject_string("tursodb: invalid handle"),
        }
    });
    raw
}

#[cfg(test)]
mod tests {
    // Unit tests for tursodb need a tokio runtime — perry-ffi's
    // spawn_blocking pumps the global runtime which is owned by
    // perry-stdlib's async_bridge. That static isn't initialized
    // in standalone unit tests (no perry-stdlib link). End-to-end
    // smoke testing happens via the TS integration in release
    // mode, where the full link surface is in place.
    //
    // The pure-Rust correctness of the underlying turso crate is
    // covered by upstream tests; our wrapper just plumbs args
    // and resolutions, exercised end-to-end.
}
