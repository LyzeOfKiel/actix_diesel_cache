use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::{PhantomData, Unpin};
use std::sync::{Arc, RwLock};

use actix::prelude::*;

use diesel::associations::HasTable;
use diesel::connection::Connection;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::sql_types::HasSqlType;

use crate::{Cache, ConnBackend, Result};

/// Save one entry
#[derive(Debug, Message)]
#[rtype(result = "Result<()>")]
pub struct Save<T>(pub T);

/// Save one entry
#[cfg(feature = "postgres")]
#[derive(Debug, Message)]
#[rtype(result = "Result<C>")]
pub struct SaveWithResult<Conn, Table, W, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    /// Data to write
    pub w: W,
    _c: PhantomData<C::Row>,
}

impl<Conn, Table, W, C> SaveWithResult<Conn, Table, W, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    /// Constructor
    pub fn new(w: W) -> Self {
        Self { w, _c: PhantomData }
    }
}

/// Gets item by id
#[derive(Debug, Message)]
#[rtype(result = "Result<Option<C>>")]
pub struct Get<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    /// Id of item to get
    pub id: C::Id,
}

impl<Conn, Table, C> Clone for Get<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
    C::Id: Clone,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
        }
    }
}

impl<Conn, Table, C> Copy for Get<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
    C::Id: Clone + Copy,
{
}

/// Gets all entries
#[derive(Debug, Clone, Copy, Message)]
#[rtype(result = "Result<Arc<RwLock<HashMap<C::Id, C>>>>")]
pub struct GetAll<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table> + 'static,
{
    _c: PhantomData<(Conn, Table, C)>,
}

impl<Conn, Table, C> Default for GetAll<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table> + 'static,
{
    fn default() -> Self {
        GetAll {
            _c: Default::default(),
        }
    }
}
