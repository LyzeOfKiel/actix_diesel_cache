use actix::Actor;
use diesel::{prelude::*, table};
#[macro_use]
extern crate diesel;

table! {
    shop (id) {
        id -> Integer,
        name -> Text,
        address -> Text,
    }
}

#[derive(Queryable, Insertable, Clone, Debug, Eq, PartialEq)]
#[table_name = "shop"]
struct Shop {
    id: i32,
    name: String,
    address: String,
}

impl actix_diesel_cache::Cache<PgConnection, shop::table> for Shop {
    type Id = i32;
    fn get_id(&self) -> Self::Id {
        self.id
    }
}

async fn example(conn: PgConnection) -> actix_diesel_cache::Result<()> {
    println!("Conn");
    let addr = actix_diesel_cache::CacheDbActor::new(conn)?.start();

    let shop = Shop {
        id: 1,
        name: String::from("Nike"),
        address: String::from("Central street"),
    };

    println!("Save Returning");
    let shop1: Shop = addr
        .send(actix_diesel_cache::SaveWithResult(shop.clone()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(shop, shop1);

    println!("Save Default");
    addr.send(actix_diesel_cache::Save(shop.clone()))
        .await
        .unwrap()?;

    println!("Get 1");
    let shop1: Shop = addr
        .send(actix_diesel_cache::Get { id: 1 })
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(shop, shop1);

    println!("Get all");
    let shops = addr
        .send(actix_diesel_cache::GetAll::default())
        .await
        .unwrap()?;

    assert_eq!(shops.read().unwrap().get(&1).unwrap(), &shop);

    Ok(())
}

fn establish_connection() -> PgConnection {
    let database_url = String::from("postgres://postgres:postgres@localhost:5432");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

fn main() -> std::io::Result<()> {
    actix::run(async move {
        let conn = establish_connection();
        example(conn).await.expect("123");
    })
}
