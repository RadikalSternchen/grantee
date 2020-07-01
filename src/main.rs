#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate validator_derive;
extern crate validator;

use rocket::fairing::AdHoc;
use rocket::request::{LenientForm, Form, FlashMessage};
use rocket::response::{Flash, Responder, Redirect, status};
use rocket::http::{Cookie, Cookies};
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
mod auth;
pub struct Database(sled::Db);

#[get("/")]
fn index(flash: Option<FlashMessage>, user: Option<auth::User>) -> Template {
    let context = default_context(flash, user);
    Template::render("index", &context)
}


#[get("/login")]
fn already_logged_in(_user: auth::User) -> Redirect {
    Redirect::to(uri!(list))
}

#[get("/logout")]
fn logout(_user: auth::User, mut cookies: Cookies) -> Redirect {
    cookies.remove_private(Cookie::named("username"));
    Redirect::to(uri!(index))
}

#[get("/login", rank=2)]
fn login() -> Template {
    let mut context = Context::new();
    context.insert("form", &auth::LoginForm::default());
    context.insert("errors", "");
    Template::render("auth/login", &context)
}

#[post("/login",  data = "<login>")]
fn post_login(login: Form<auth::LoginForm>, db: State<auth::UserDatabase>, mut cookies: Cookies) -> Result<Redirect, Template> {
    if db.login(&login) {
        cookies.add_private(
            Cookie::build("username", login.username().clone())
            .http_only(true)
            .secure(true)
            .finish()
        );
        return Ok(Redirect::to(uri!(list)))
    } 
    
    let mut context = Context::new();
    let mut msg = HashMap::new();
    msg.insert("msg", "Login failed");
    msg.insert("name", "error");
    context.insert("flash_message", &msg);

    context.insert("form", &login.into_inner());
    context.insert("errors", "");
    Err(Template::render("auth/login", &context))
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

fn default_context(flash: Option<FlashMessage>, user: Option<auth::User>) -> Context {
    let mut context = Context::new();
    if let Some(msg) = flash {
        let m: HashMap<&str, &str> = vec![
            ("name", msg.name()),
            ("msg", msg.msg())
        ].into_iter().collect();
        context.insert("flash_message", &m);
    }
    if let Some(user) = user {
        context.insert("current_user", &user.username());
    }  else {
        context.insert("current_user", &false);
    }

    context
}

#[get("/list")]
fn list(database: State<Database>, flash: Option<FlashMessage>, user: auth::User)
    -> Template
{
    let mut context = default_context(flash, Some(user));
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
    Template::render("grants/list", &context)
}

#[post("/v/<id>?<next>", data = "<form>")]
fn update_grant(
    id: String,
    next: Option<String>,
    form: Form<model::NextStageForm>,
    db: State<Database>,
    user: auth::User,
)  -> Result<Flash<Redirect>, status::BadRequest<Template>> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e|status::BadRequest(Some(render_error(e.to_string()))))?;
    let mut grant =  get_or_404(&db, uuid.as_bytes())
        .map_err(|e| status::BadRequest(Some(e.0)))?;

    grant.next_stage(user.username().clone(), form.into_inner())
        .map_err(|e|status::BadRequest(Some(render_error(e.to_string()))))?;

    db.0.insert(uuid.as_bytes(), grant.encode())
        .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))?;

    db.0.flush()
        .map_err(|e| status::BadRequest(Some(render_error(e.to_string()))))?;

    let id = uuid.to_string();
    let redir = match next {
        Some(v) => Redirect::to(v),
        None => Redirect::to(uri!(view_grant: id))
    };

    Ok(Flash::success(redir, format!(
        "Antrag '{:}' ist jetzt '{:}'",
        grant.title().expect("We have a grant"),
        grant.state_name().expect("We are a grant")
    )))
}

#[get("/v/<id>")]
fn view_grant(id: String, database: State<Database>, flash: Option<FlashMessage>, user: Option<auth::User>)
    -> Result<Template, status::NotFound<Template>>
{
    let uuid = Uuid::parse_str(&id).map_err(|e|status::NotFound(render_error(e.to_string())))?;
    let grant =  get_or_404(&database, uuid.as_bytes())?;
    
    let mut context = default_context(flash, user);
    context.insert("uuid", &uuid.to_string());

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
        .attach(AdHoc::on_attach("Login Config", |rocket| {
            let db: HashMap<String, String> = rocket.config()
                .get_table("users")
                .map(|t| t
                        .into_iter()
                        .filter_map(|(k, v)| v.clone().try_into::<String>().ok().map(|r| (k.clone(), r)))
                        .collect())
                .unwrap_or_default();
            Ok(rocket.manage(auth::UserDatabase::new(db)))
        }))
        .mount("/pub", StaticFiles::from("static"))
        .mount("/", routes![
            index,
            already_logged_in,
            post_login,
            login,
            logout,

            list,
            view_grant,
            update_grant,
            new_event_grant_post,
            new_event_grant,
        ])
        .launch();
}
