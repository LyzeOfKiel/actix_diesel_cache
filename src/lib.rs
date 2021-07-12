//! actix_diesel_cache is crate which provides the actix actor for caching all
//! database entries on local machine.

#![deny(missing_docs)]

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::{PhantomData, Unpin};
use std::sync::{Arc, RwLock};

use actix::prelude::*;

use diesel::associations::HasTable;
use diesel::backend::Backend;
use diesel::backend::SupportsReturningClause;
use diesel::connection::Connection;
use diesel::deserialize::Queryable;
use diesel::insertable::CanInsertInSingleQuery;
#[cfg(feature = "postgres")]
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::sql_types::HasSqlType;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;

/// Messages for cache actor
pub mod messages;
use messages::*;

/// Error of cache actor
pub type Error = diesel::result::Error;

/// Result
pub type Result<V> = std::result::Result<V, Error>;

/// DefaultConnBackend
pub trait DefaultConnBackend<T: diesel::Table + AsQuery>: Backend {}

/// ReturningConnBackend
pub trait ReturningConnBackend<T: AsQuery + diesel::Table>:
    DefaultConnBackend<T> + SupportsReturningClause
{
}

#[cfg(feature = "sqlite")]
/// ConnBackend
pub trait ConnBackend<T: diesel::Table + AsQuery>: DefaultConnBackend<T> {}

#[cfg(feature = "postgres")]
/// ConnBackend
pub trait ConnBackend<T: diesel::Table>: ReturningConnBackend<T> {}

#[cfg(feature = "sqlite")]
impl<T: diesel::Table + AsQuery> DefaultConnBackend<T> for Sqlite {}
#[cfg(feature = "postgres")]
impl<T: diesel::Table + AsQuery> DefaultConnBackend<T> for Pg {}

#[cfg(feature = "sqlite")]
impl<T: diesel::Table + AsQuery> ConnBackend<T> for Sqlite {}

#[cfg(feature = "postgres")]
impl<T: diesel::Table + AsQuery> ReturningConnBackend<T> for Pg {}

#[cfg(feature = "postgres")]
impl<T: diesel::Table + AsQuery> ConnBackend<T> for Pg {}

/// Trait for CacheDbActor. Requires at compile time for type to be queryable in
/// table and database backend.
///
/// Connection backend should have all types in table.
pub trait Cache<Conn, Table>:
    Queryable<Table::SqlType, Conn::Backend> + Sized + Debug + Clone + 'static
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
{
    /// Id type for getting specific records
    type Id: Hash + Eq + Clone;

    /// Get id of item
    fn get_id(&self) -> Self::Id;

    /// Read all entries from db
    fn read_all(c: &Conn) -> Result<HashMap<Self::Id, Self>> {
        let vec: Vec<Self> = Table::table().load(c)?;
        let mut out = HashMap::with_capacity(vec.len());
        for it in vec {
            let id = it.get_id();
            out.insert(id, it);
        }
        Ok(out)
    }

    #[cfg(feature = "postgres")]
    /// Write one entry to db returning affected row.
    ///
    /// Entry type should be insertable in table and its sqltype should be
    /// insertable in one query.
    fn write_one_with_result<C, W>(w: W, c: &Conn) -> Result<C>
    where
        Table::FromClause: QueryFragment<Conn::Backend>,
        W: Insertable<Table>,
        W::Values: CanInsertInSingleQuery<Conn::Backend> + QueryFragment<Conn::Backend>,
        C: Cache<Conn, Table>
            + diesel::Queryable<
                <<Table as diesel::Table>::AllColumns as diesel::Expression>::SqlType,
                <Conn as diesel::Connection>::Backend,
            >,
        Table::AllColumns: QueryFragment<Conn::Backend>,
        Conn::Backend: ConnBackend<Table>
            + SupportsReturningClause
            + HasSqlType<<Table::AllColumns as diesel::Expression>::SqlType>,
    {
        diesel::insert_into(Table::table()).values(w).get_result(c)
    }

    /// Write one entry to db.
    ///
    /// Entry type should be insertable in table and its sqltype should be
    /// insertable in one query.
    fn write_one<W>(w: W, c: &Conn) -> Result<usize>
    where
        Table::FromClause: QueryFragment<Conn::Backend>,
        W: Insertable<Table>,
        W::Values: CanInsertInSingleQuery<Conn::Backend> + QueryFragment<Conn::Backend>,
    {
        diesel::insert_into(Table::table()).values(w).execute(c)
    }
}

/// Actix Actor for caching database.
/// Has fast reads and slow writes. Updates its records once in a minute and on inserts.
pub struct CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    /// Connection for db
    conn: Conn,
    /// All items read from db
    cache: Arc<RwLock<HashMap<C::Id, C>>>,
    /// Cache valid
    is_valid: bool,
    /// Phantom marker for saving table inside structure
    t: PhantomData<Table>,
}

impl<Conn, Table, C> CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    /// Constructor
    pub fn new(conn: Conn) -> Result<Self> {
        let (cache, t) = Default::default();
        let mut s = Self {
            conn,
            cache,
            is_valid: true,
            t,
        };
        s.update()?;
        Ok(s)
    }

    fn update(&mut self) -> Result<()> {
        self.cache = Arc::new(RwLock::new(C::read_all(&self.conn)?));
        Ok(())
    }

    fn update_one(&mut self, id: C::Id, v: C) -> Option<C> {
        let mut cache_guard = self.cache.write().unwrap();
        (*cache_guard).insert(id, v)
    }

    fn get(&self, id: C::Id) -> Option<C> {
        let cache_guard = self.cache.read().unwrap();
        (*cache_guard).get(&id).cloned()
    }

    fn timer_update(&mut self, context: &mut Context<Self>) {
        let dur = std::time::Duration::from_secs(60);
        let _ = self.update();
        TimerFunc::new(dur, Self::timer_update).spawn(context);
    }
}

impl<Conn, Table, C> Actor for CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    type Context = Context<Self>;

    fn started(&mut self, context: &mut Context<Self>) {
        self.timer_update(context)
    }
}

impl<Conn, Table, C> Handler<GetAll<Conn, Table, C>> for CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    type Result = Result<Arc<RwLock<HashMap<C::Id, C>>>>;

    fn handle(&mut self, _: GetAll<Conn, Table, C>, _: &mut Context<Self>) -> Self::Result {
        if !self.is_valid {
            // Flushing not by timer because we are not supposed to have error in
            // exported data.
            self.update()?;
        }
        Ok(Arc::clone(&self.cache))
    }
}

#[cfg(feature = "postgres")]
impl<Conn, Table, W, C> Handler<SaveWithResult<Conn, Table, W, C>> for CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table>
        + HasSqlType<Table::SqlType>
        + HasSqlType<<Table::AllColumns as diesel::Expression>::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    Table::FromClause: QueryFragment<Conn::Backend>,
    Table::AllColumns: QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>
        + diesel::Queryable<
            <<Table as diesel::Table>::AllColumns as diesel::Expression>::SqlType,
            <Conn as diesel::Connection>::Backend,
        >,
    W: Insertable<Table>,
    W::Values: CanInsertInSingleQuery<Conn::Backend> + QueryFragment<Conn::Backend>,
{
    type Result = Result<C>;

    fn handle(
        &mut self,
        pred: SaveWithResult<Conn, Table, W, C>,
        _: &mut Context<Self>,
    ) -> Self::Result {
        let row = C::write_one_with_result(pred.w, &self.conn)?;
        self.update_one(C::get_id(&row), row.clone());
        Ok(row)
    }
}

impl<Conn, Table, C, W> Handler<Save<W>> for CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    Table::FromClause: QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
    W: Insertable<Table> + 'static,
    W::Values: CanInsertInSingleQuery<Conn::Backend> + QueryFragment<Conn::Backend>,
{
    type Result = Result<()>;

    fn handle(&mut self, pred: Save<W>, _: &mut Context<Self>) -> Self::Result {
        self.is_valid = false;
        C::write_one(pred.0, &self.conn)?;
        self.update()?;
        Ok(())
    }
}

impl<Conn, Table, C> Handler<Get<Conn, Table, C>> for CacheDbActor<Conn, Table, C>
where
    Conn: Connection + Unpin + 'static,
    Conn::Backend: ConnBackend<Table> + HasSqlType<Table::SqlType>,
    Table: diesel::Table + HasTable<Table = Table> + AsQuery + Unpin + 'static,
    Table::Query: QueryId + QueryFragment<Conn::Backend>,
    C: Cache<Conn, Table>,
{
    type Result = Result<Option<C>>;

    fn handle(&mut self, Get { id }: Get<Conn, Table, C>, _: &mut Context<Self>) -> Self::Result {
        match self.get(id.clone()) {
            Some(out) => Ok(Some(out)),
            None => {
                self.update()?;
                Ok(self.get(id))
            }
        }
    }
}
