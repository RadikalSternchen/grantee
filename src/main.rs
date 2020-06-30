#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate validator_derive;
extern crate validator;

use rocket::fairing::AdHoc;
use rocket::request::{LenientForm, FlashMessage};
use rocket::response::{Flash, Redirect, status};
use validator::Validate;
use parity_scale_codec::Encode;

use std::collections::HashMap;
use sled;
use rocket::State;
use rocket_contrib::{
    templates::{Template, tera::Context},
    serve::StaticFiles,
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
    let mut context = Context::new();
    context.insert("form", &model::EventGrantForm::default());
    context.insert("errors", "");
    Template::render("grants/event_grant_form", &context)
}

fn render_error(e: String) -> Template {
    let mut context = Context::new();
    context.insert("error", &e);
    Template::render("generic/error", &context)
}

#[post("/event-grants/new", data = "<event>")]
fn new_event_grant_post(event: LenientForm<model::EventGrantForm>, database: State<Database>)
    -> Result<Flash<Redirect>, status::BadRequest<Template>>
{
    let event = event.into_inner();
    match event.validate() {
        Ok(_) => {
            println!("all good");
            let db_item = model::Model::from(event);
            loop {
                let id = uuid::Uuid::new_v4();
                if database.0.contains_key(id.as_bytes()).unwrap_or(false) { continue }

                return database.0.insert(id.as_bytes(), db_item.encode())
                    .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))
                    .map(|_|
                        Flash::success(
                            Redirect::to(uri!(view_grant: id.to_string())),
                            "Dein Antrag ist eingegangen. Danke!"
                        )
                    )
            }
        }
        Err(errors) => {
            let mut context =  Context::new();
            println!("{:#?} –> {:#?}", event, errors);
            context.insert("form", &event);
            context.insert("errors", &errors);

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
fn view_grant(id: String, database: State<Database>, flash: Option<FlashMessage>) -> Result<Template, status::NotFound<String>>
{
    let mut context = Context::new();
    if let Some(msg) = flash {
        let m: HashMap<&str, &str> = vec![
            ("name", msg.name()),
            ("msg", msg.msg())
        ].into_iter().collect();
        context.insert("flash_message", &m);
    }
    Ok(Template::render("grants/view_grant", &context))
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
