mod juju;

fn main() {
    let engine = std::env::args()
        .nth(1)
        .expect("no engine given (should be one of: juju)");
    let available_applications = match engine.as_str() {
        "juju" => juju::list_applications(),
        _ => {
            println!("unknown engine: {}", engine);
            std::process::exit(1);
        }
    };
    let application = std::env::args().nth(2).expect("no application given");
    if !available_applications.contains(&application) {
        println!("unknown application: {}", application);
        std::process::exit(1);
    }
    println!("engine: {:?}, application: {:?}", engine, application)
}
