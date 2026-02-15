use serde_json::Value;

const TMDB_BASE: &str = "https://api.themoviedb.org";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p/w342";

#[derive(Clone)]
pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn search_movie_poster(&self, title: &str, year: Option<i64>) -> Option<String> {
        let mut params = vec![("api_key", self.api_key.as_str()), ("query", title)];
        let year_str = year.map(|y| y.to_string());
        if let Some(ref y) = year_str {
            params.push(("year", y));
        }

        let resp = self
            .client
            .get(format!("{TMDB_BASE}/3/search/movie"))
            .query(&params)
            .send()
            .await
            .ok()?;

        let json: Value = resp.json().await.ok()?;
        json["results"]
            .as_array()?
            .first()?
            .get("poster_path")?
            .as_str()
            .map(|s| s.to_string())
    }

    pub async fn search_tv_poster(&self, title: &str) -> Option<String> {
        let params = [("api_key", self.api_key.as_str()), ("query", title)];

        let resp = self
            .client
            .get(format!("{TMDB_BASE}/3/search/tv"))
            .query(&params)
            .send()
            .await
            .ok()?;

        let json: Value = resp.json().await.ok()?;
        json["results"]
            .as_array()?
            .first()?
            .get("poster_path")?
            .as_str()
            .map(|s| s.to_string())
    }
}

pub fn poster_url(poster_path: &str) -> String {
    format!("{TMDB_IMAGE_BASE}{poster_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poster_url_builds_correct_url() {
        assert_eq!(
            poster_url("/abc123.jpg"),
            "https://image.tmdb.org/t/p/w342/abc123.jpg"
        );
    }
}
