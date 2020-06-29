#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use]
extern crate validator_derive;
extern crate validator;

use rocket::fairing::AdHoc;
use std::collections::HashMap;
use sled;
use rocket::State;
use rocket_contrib::{
    templates::Template,
    serve::StaticFiles,
    uuid::Uuid,
};

mod model;
struct Database(sled::Db);


// impl Database {
//     fn insert<K:Encode, V: Encode>(key: K, mdl: V) -> Result<(), String> {
//         self.0
//     }

//     fn get<K:Encode, M: Decode>(key: K) -> Result<M, String> {
//         M::decode(self.0.get(key))
//     }
// }

#[get("/")]
fn index() -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("example", &context)
}

#[get("/view/<id>")]
fn view_ext(id: Uuid, database: State<Database>) -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("example", &context)
}



fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .attach(AdHoc::on_attach("Database Config", |rocket| {
            let db_dir = rocket.config()
                .get_str("database")
                .unwrap_or("sled")
                .to_string();
            
            let db = sled::open(db_dir)
                .expect("Opening sled database failed");
            Ok(rocket.manage(Database(db)))
        }))
        .mount("/pub", StaticFiles::from("static"))
        .mount("/", routes![index])
        .launch();
}
