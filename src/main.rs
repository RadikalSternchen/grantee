#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate validator_derive;
extern crate validator;

use rocket::fairing::AdHoc;
use rocket::request::{LenientForm, FlashMessage};
use rocket::response::{Flash, Redirect, status};
use validator::Validate;
use parity_scale_codec::{Encode, Decode};
use uuid::Uuid;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
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
                let id = Uuid::new_v4();
                if database.0.contains_key(id.as_bytes()).unwrap_or(false) { continue }

                return database.0.insert(id.as_bytes(), db_item.encode())
                    .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))
                    .map(|_| {
                        database.0.flush();
                        Flash::success(
                            Redirect::to(uri!(view_grant: id.to_string())),
                            "Dein Antrag ist eingegangen. Danke!"
                        )
                    })
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

fn get_or_404(database: &Database, id: &[u8]) -> Result<model::Model, status::NotFound<Template>> {
    match database.0.get(id) {
        Ok(Some(m)) => model::Model::decode(&mut m.as_ref()).map_err(|e|
                status::NotFound(render_error(format!("Error decoding item: {:}", e)))),
        Ok(None) => Err(status::NotFound(render_error("Entry not found".to_string()))),
        Err(e) => Err(status::NotFound(render_error(e.to_string()))),
    }
}

#[get("/list")]
fn list(database: State<Database>, flash: Option<FlashMessage>)
    -> Template
{
    let mut context = Context::new();
    let entries = database.0.iter()
    .filter_map(|r|r.ok())
    .filter_map(|(uuid, val)|
        Uuid::from_slice(uuid.as_ref())
            .ok()
            .and_then(|u| model::Model::decode(&mut val.as_ref()).map(|m| (u.to_string(), m)).ok())
    ).fold(HashMap::<&'static str, Vec<(String, model::Model)>>::new(), 
    |mut m, (uuid, model)| {
        if let Some(target) = model.state_name() {
            match m.entry(target) {
                Entry::Occupied(mut o) => { o.get_mut().push((uuid, model)); },
                Entry::Vacant(v) => { v.insert(vec![(uuid, model)]); }
            }
        }
        m
    });
    context.insert("entries", &entries);
    if let Some(msg) = flash {
        let m: HashMap<&str, &str> = vec![
            ("name", msg.name()),
            ("msg", msg.msg())
        ].into_iter().collect();
        context.insert("flash_message", &m);
    }
    
    Template::render("grants/list", &context)
}

#[get("/v/<id>")]
fn view_grant(id: String, database: State<Database>, flash: Option<FlashMessage>)
    -> Result<Template, status::NotFound<Template>>
{
    let uuid = Uuid::parse_str(&id).map_err(|e|status::NotFound(render_error(e.to_string())))?;
    let grant =  get_or_404(&database, uuid.as_bytes())?;
    
    let mut context = Context::new();
    context.insert("uuid", &uuid.to_string());
    if let Some(msg) = flash {
        let m: HashMap<&str, &str> = vec![
            ("name", msg.name()),
            ("msg", msg.msg())
        ].into_iter().collect();
        context.insert("flash_message", &m);
    }

    match grant {
        model::Model::EventGrant(g) => {
            context.insert("grant", &g);
            Ok(Template::render("grants/view_event_grant", &context))
        }

        model::Model::AktivistiGrant(g) => {
            context.insert("grant", &g);
            Ok(Template::render("grants/view_aktivisti_grant", &context))
        }
        _ => Err(status::NotFound(render_error("Unknown item type".to_owned())))
    }
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
            list,
            view_grant,
            new_event_grant_post,
            new_event_grant,
        ])
        .launch();
}
