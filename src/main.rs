use crate::juju::debug;
use tokio::signal;
use tokio_util::sync::CancellationToken;

mod juju;

#[tokio::main]
async fn main() {
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let main_task_handle = tokio::spawn(async move {
        let engine = std::env::args()
            .nth(1)
            .expect("no engine given (should be one of: juju)");
        let application = std::env::args().nth(2).expect("no application given");
        match engine.as_str() {
            "juju" => {
                debug(application, token_clone).await;
            }
            _ => {
                println!("unknown engine: {}", engine);
                std::process::exit(1);
            }
        };
    });
    match signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }
    println!("waiting for tasks to finish");
    token.cancel();
    main_task_handle.await.unwrap();
}
