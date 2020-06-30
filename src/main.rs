#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use]
extern crate validator_derive;
extern crate validator;

use rocket::fairing::AdHoc;
use rocket::request::Form;
use rocket::response::{Redirect, status};
use validator::{Validate, ValidationErrors};
use parity_scale_codec::Encode;
use serde::{Serialize, Deserialize};
use std::borrow::Cow;

use std::collections::HashMap;
use sled;
use rocket::State;
use rocket_contrib::{
    templates::{Template, tera::Context},
    serve::StaticFiles,
    uuid::Uuid as rUuid,
};

mod model;
pub struct Database(sled::Db);

#[get("/")]
fn index() -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("index", &context)
}

#[get("/event-grants/new")]
fn new_event_grant() -> Template {
    let mut context =  Context::new();
    context.insert("form", &model::EventGrantForm::default());
    context.insert("errors", &HashMap::<String, String>::new());
    Template::render("grants/event_grant_form", &context)
}

#[post("/event-grants/new", data = "<event>")]
fn new_event_grant_post(event: Form<model::EventGrantForm>, database: State<Database>)
    -> Result<Redirect, status::BadRequest<Template>>
{
    let event = event.into_inner();
    match event.validate() {
        Ok(_) => {
            let db_item = model::Model::from(event);
            loop {
                let id = uuid::Uuid::new_v4();
                if database.0.contains_key(id.as_bytes()).unwrap_or(false) { continue }

                database.0.insert(id.as_bytes(), db_item.encode());
                return Ok(Redirect::temporary(uri!(view_grant: id.to_string())))
            }
        }
        Err(errors) => {
            // print!("errors: {:?}", e);
            // let errors:HashMap<&'static str, Vec<Cow<'static, str>>> = e.field_errors()
            //     .into_iter().map(|(k, errs)| {
            //         (k, errs.into_iter().map(|v| v.message.clone().unwrap_or("Error".into())).collect())
            //     }).collect();
            let mut context =  Context::new();
            context.insert("form", &event);
            context.insert("errors", &errors);
            println!("{:?}", errors);

            Err(status::BadRequest(
                Some(
                    Template::render(
                        "grants/event_grant_form",
                        &context
                    )
                )
            ))
        }
    }
}

#[get("/v/<id>")]
fn view_grant(id: String, database: State<Database>) -> Template {
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
        .mount("/", routes![
            index,
            view_grant,
            new_event_grant_post,
            new_event_grant,
        ])
        .launch();
}
