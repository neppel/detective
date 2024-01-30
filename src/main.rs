use crate::juju::debug;

mod juju;

#[tokio::main]
async fn main() {
    let engine = std::env::args()
        .nth(1)
        .expect("no engine given (should be one of: juju)");
    let application = std::env::args().nth(2).expect("no application given");
    match engine.as_str() {
        "juju" => {
            debug(application.clone()).await;
        }
        _ => {
            println!("unknown engine: {}", engine);
            std::process::exit(1);
        }
    };
}
