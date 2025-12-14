// Example demonstrating how to use EntityIDFor<T> with Rocket forms

use rocket::form::Form;
use rocket::{get, launch, post, routes, FromForm};
use terminusdb_schema::EntityIDFor;
use terminusdb_schema::{Schema, ToTDBSchema};

// Define a sample entity type
#[derive(Clone, Debug)]
struct User;

impl ToTDBSchema for User {
    fn schema_name() -> String {
        "User".to_string()
    }

    fn to_schema_tree() -> Vec<Schema> {
        vec![] // Simplified for example
    }
}

// Define a form struct that uses EntityIDFor
#[derive(FromForm)]
struct UserUpdateForm {
    user_id: EntityIDFor<User>,
    name: String,
    email: String,
}

// Route that accepts the form
#[post("/update-user", data = "<form>")]
fn update_user(form: Form<UserUpdateForm>) -> String {
    let user_form = form.into_inner();
    format!(
        "Updating user: {} (ID: {}) with name: {} and email: {}",
        user_form.user_id.typed(),
        user_form.user_id.id(),
        user_form.name,
        user_form.email
    )
}

// Example route using EntityIDFor as a path parameter
#[get("/user/<user_id>")]
fn get_user(user_id: EntityIDFor<User>) -> String {
    format!(
        "Getting user with ID: {} (typed: {})",
        user_id.id(),
        user_id.typed()
    )
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![update_user, get_user])
}

// Example form submissions that would work:
//
// 1. Simple ID:
// user_id=123&name=John&email=john@example.com
//
// 2. Typed ID:
// user_id=User/123&name=John&email=john@example.com
//
// 3. Full IRI:
// user_id=terminusdb://data#User/123&name=John&email=john@example.com
