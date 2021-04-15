use tide::{utils::async_trait, Middleware, Next, Request, Result};

pub struct LogErrors;

#[async_trait]
impl<State: Clone + Send + Sync + 'static> Middleware<State> for LogErrors {
    async fn handle(&self, request: Request<State>, next: Next<'_, State>) -> Result {
        let method = request.method();
        let url = request.url().clone();
        let response = next.run(request).await;
        if let Some(err) = response.error() {
            eprintln!("Error handling request {} {}: {}", method, url, err);
        }
        Ok(response)
    }
}
