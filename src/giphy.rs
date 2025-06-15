use crate::db::Database;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyResponse {
    pub data: Vec<GiphyGif>,
    pub pagination: GiphyPagination,
    pub meta: GiphyMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyGif {
    pub id: String,
    pub title: String,
    pub rating: String,
    pub images: GiphyImages,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyImages {
    pub original: GiphyImage,
    pub fixed_height: GiphyImage,
    pub fixed_width: GiphyImage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyImage {
    pub url: String,
    pub width: String,
    pub height: String,
    pub size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyPagination {
    pub total_count: i32,
    pub count: i32,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiphyMeta {
    pub status: i32,
    pub msg: String,
    pub response_id: String,
}

pub struct GiphyClient {
    client: Client,
    api_key: String,
    db: Database,
}

impl GiphyClient {
    pub fn new(db: Database) -> Result<Self> {
        let api_key = env::var("GIPHY_API_KEY")?;
        let client = Client::new();

        Ok(Self {
            client,
            api_key,
            db,
        })
    }

    pub async fn search(&self, query: &str, limit: u32, offset: u32) -> Result<GiphyResponse> {
        let url = "https://api.giphy.com/v1/gifs/search";

        let response = self
            .client
            .get(url)
            .query(&[
                ("api_key", &self.api_key),
                ("q", &query.to_string()),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
                ("rating", &"pg-13".to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("GIPHY API error: {}", response.status());
        }

        let giphy_response = response.json::<GiphyResponse>().await?;
        Ok(giphy_response)
    }

    pub async fn get_top_gifs(&self, query: &str, limit: u32) -> Result<Vec<GiphyGif>> {
        // Always get from offset 0 to get the most relevant results
        let response = self.search(query, limit, 0).await?;
        Ok(response.data)
    }

    pub async fn get_random_with_cache(
        &self,
        exclude_id: Option<&str>,
    ) -> Result<Option<GiphyGif>> {
        // Get active search terms from database
        let search_terms = self.db.get_active_giphy_search_terms().await?;

        if search_terms.is_empty() {
            warn!("No active GIPHY search terms found in database");
            return Ok(None);
        }

        // Pick a random search term
        let search_term = &search_terms[rand::random::<usize>() % search_terms.len()];

        // First, try to get from cache
        if let Some(cached_gif) = self
            .db
            .get_cached_giphy_gif(search_term, exclude_id)
            .await?
        {
            info!("Returning cached GIPHY result for term: {}", search_term);
            return Ok(Some(cached_gif));
        }

        // If not in cache or cache is empty, fetch from API
        info!(
            "No cached results for '{}', fetching from GIPHY API",
            search_term
        );

        match self.fetch_and_cache(search_term).await {
            Ok(gif) => Ok(gif),
            Err(e) => {
                warn!("Failed to fetch from GIPHY API: {}", e);
                // Try another search term if available
                if search_terms.len() > 1 {
                    let alt_term =
                        &search_terms[(rand::random::<usize>() + 1) % search_terms.len()];
                    if let Some(cached) = self.db.get_cached_giphy_gif(alt_term, exclude_id).await?
                    {
                        return Ok(Some(cached));
                    }
                }
                Ok(None)
            }
        }
    }

    async fn fetch_and_cache(&self, search_term: &str) -> Result<Option<GiphyGif>> {
        // Always fetch top 10 most relevant results
        const TOP_RESULTS_LIMIT: u32 = 10;

        let gifs = self.get_top_gifs(search_term, TOP_RESULTS_LIMIT).await?;

        if gifs.is_empty() {
            return Ok(None);
        }

        // Cache all top results for future use
        for gif in &gifs {
            if let Err(e) = self.db.cache_giphy_gif(search_term, gif).await {
                warn!("Failed to cache GIF {}: {}", gif.id, e);
            }
        }

        info!(
            "Cached {} top GIFs for search term '{}'",
            gifs.len(),
            search_term
        );

        // Return a random one from the results
        let index = rand::random::<usize>() % gifs.len();
        Ok(gifs.into_iter().nth(index))
    }
}
