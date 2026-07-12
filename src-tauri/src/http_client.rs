/// Shared HTTP client wrapper
#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new() }
    }

    pub fn get(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.get(url)
    }

    pub fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.post(url)
    }

    pub fn put(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.put(url)
    }

    pub fn delete(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.delete(url)
    }

    pub fn raw_client(&self) -> &reqwest::Client {
        &self.client
    }
}
