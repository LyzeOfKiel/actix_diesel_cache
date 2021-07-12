use std::{collections::HashMap, sync::RwLockReadGuard};

use actix::{Actor, Addr};
use actix_diesel_cache::{messages::*, CacheDbActor};
use diesel::{table, PgConnection};
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod db;
use db::PgDb;

embed_migrations!("./tests/migrations/");

table! {
    shop (id) {
        id -> Integer,
        name -> Text,
        address -> Text,
    }
}

#[derive(Queryable, Insertable, Clone, Debug, Eq, PartialEq)]
#[table_name = "shop"]
pub struct Shop {
    id: i32,
    name: String,
    address: String,
}

#[derive(Queryable, Insertable, Clone, Debug)]
#[table_name = "shop"]
pub struct ShopInsert {
    name: String,
    address: String,
}

pub type ConnectionManager = diesel::r2d2::ConnectionManager<PgConnection>;
pub type Pool = diesel::r2d2::Pool<ConnectionManager>;
pub type PooledConnection = diesel::r2d2::PooledConnection<ConnectionManager>;

impl actix_diesel_cache::Cache<PooledConnection, shop::table> for Shop {
    type Id = i32;
    fn get_id(&self) -> Self::Id {
        self.id
    }
}

/// Inits pool of connections to database
pub fn init_db_pool(db_url: &str) -> Pool {
    Pool::builder()
        .connection_timeout(std::time::Duration::from_secs(10))
        .max_size(5)
        .build(ConnectionManager::new(db_url))
        .unwrap()
}

pub struct CacheWrap {
    pub addr: Addr<CacheDbActor<PooledConnection, shop::table, Shop>>,
    pub db: PgDb,
}

fn setup() -> CacheWrap {
    let db = PgDb::new();
    let pool = init_db_pool(db.url.as_str());

    let conn = pool.get().unwrap();
    embedded_migrations::run(&conn).unwrap();

    let conn = pool.get().unwrap();

    let addr = actix_diesel_cache::CacheDbActor::new(conn).unwrap().start();
    CacheWrap { addr, db }
}

#[actix_rt::test]
async fn save_works() {
    let wrap = setup();

    let shop1 = ShopInsert {
        name: String::from("Nike"),
        address: String::from("Central street"),
    };

    let shop2 = ShopInsert {
        name: String::from("Adidas"),
        address: String::from("Some street"),
    };

    wrap.addr.send(Save(shop1.clone())).await.unwrap().unwrap();
    wrap.addr.send(Save(shop2.clone())).await.unwrap().unwrap();

    let shops = wrap.addr.send(GetAll::default()).await.unwrap().unwrap();
    let shops: RwLockReadGuard<HashMap<_, Shop>> = shops.read().unwrap();

    assert!(shops.len() == 2);

    let shop = shops.get(&1).unwrap();
    assert_eq!(shop.name, shop1.name);
    assert_eq!(shop.address, shop1.address);
    let shop = shops.get(&2).unwrap();
    assert_eq!(shop.name, shop2.name);
    assert_eq!(shop.address, shop2.address);
}

#[actix_rt::test]
async fn savewithresult_works() {
    let wrap = setup();

    let shop1 = ShopInsert {
        name: String::from("Nike"),
        address: String::from("Central street"),
    };

    let shop: Shop = wrap
        .addr
        .send(SaveWithResult::new(shop1.clone()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(shop.name, shop1.name);
    assert_eq!(shop.address, shop1.address);

    let shop: Shop = wrap
        .addr
        .send(Get { id: 1 })
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(shop.name, shop1.name);
    assert_eq!(shop.address, shop1.address);
}
