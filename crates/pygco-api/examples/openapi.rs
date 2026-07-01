fn main() {
    let document = pygco_api::openapi_document();
    println!("{}", serde_json::to_string_pretty(&document).unwrap());
}
