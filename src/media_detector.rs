use regex::Regex;
use std::collections::HashMap;

pub struct MediaRecommendation {
    pub media_type: &'static str,
    pub title: String,
    pub url: Option<String>,
    pub confidence: f32,
}

pub struct MediaDetector {
    anime_patterns: Vec<Regex>,
    tv_show_patterns: Vec<Regex>,
    game_patterns: Vec<Regex>,
    youtube_pattern: Regex,
    url_pattern: Regex,
}

impl MediaDetector {
    pub fn new() -> Self {
        Self {
            // Anime patterns
            anime_patterns: vec![
                Regex::new(r"(?i)(?:watching|watched|recommend|check out|love|enjoying)\s+(?:the\s+)?anime\s+([A-Za-z0-9\s:\-!?']+?)(?:\s+(?:is|was|it's|its|season|episode|ep\s*\d+)|[.!?,]|$)").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+(?:is|was)\s+(?:a\s+)?(?:great|good|amazing|awesome|fantastic)\s+anime").unwrap(),
                Regex::new(r"(?i)(?:started|finished|binged?)\s+([A-Za-z0-9\s:\-!?']+?)\s+(?:anime|last night|today|yesterday)").unwrap(),
                Regex::new(r"(?i)(?:anime\s+)?([A-Za-z0-9\s:\-!?']+?)\s+(?:season|S)\s*(\d+)").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+ep(?:isode)?\s*\d+").unwrap(),
            ],
            
            // TV show patterns
            tv_show_patterns: vec![
                Regex::new(r"(?i)(?:watching|watched|recommend|check out|binged?)\s+(?:the\s+)?(?:show|series|season)\s+([A-Za-z0-9\s:\-!?']+?)(?:\s+(?:on|is|was)|[.!?,]|$)").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+(?:on|is on)\s+(?:Netflix|Hulu|HBO|Disney\+|Amazon Prime|Apple TV)").unwrap(),
                Regex::new(r"(?i)(?:just\s+)?(?:started|finished)\s+([A-Za-z0-9\s:\-!?']+?)\s+(?:season|S)\s*\d+").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+(?:is|was)\s+(?:such\s+)?(?:a\s+)?(?:great|good|amazing|awesome)\s+(?:show|series)").unwrap(),
            ],
            
            // Game patterns
            game_patterns: vec![
                Regex::new(r"(?i)(?:playing|played|recommend|check out|love|enjoying)\s+(?:the\s+)?(?:game\s+)?([A-Za-z0-9\s:\-!?']+?)(?:\s+(?:is|was|it's|on)|[.!?,]|$)").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+(?:is|was)\s+(?:such\s+)?(?:a\s+)?(?:great|good|amazing|awesome|fun)\s+game").unwrap(),
                Regex::new(r"(?i)(?:got|bought|downloaded)\s+([A-Za-z0-9\s:\-!?']+?)\s+(?:on|from)\s+(?:Steam|Epic|Xbox|PlayStation|Switch)").unwrap(),
                Regex::new(r"(?i)([A-Za-z0-9\s:\-!?']+?)\s+(?:gameplay|walkthrough|guide|review)").unwrap(),
            ],
            
            // YouTube pattern
            youtube_pattern: Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/shorts/)([A-Za-z0-9_\-]+)").unwrap(),
            
            // General URL pattern
            url_pattern: Regex::new(r#"https?://[^\s<>"{}|\\^`\[\]]+"#).unwrap(),
        }
    }

    pub fn detect_media(&self, content: &str) -> Vec<MediaRecommendation> {
        let mut recommendations = Vec::new();
        let mut found_titles = HashMap::new();

        // Check for anime mentions
        for pattern in &self.anime_patterns {
            for cap in pattern.captures_iter(content) {
                if let Some(title_match) = cap.get(1) {
                    let title = self.clean_title(title_match.as_str());
                    if !title.is_empty() && title.len() > 2 && !found_titles.contains_key(&title) {
                        found_titles.insert(title.clone(), "anime");

                        // Extract URL if present nearby
                        let url =
                            self.find_nearby_url(content, title_match.start(), title_match.end());

                        recommendations.push(MediaRecommendation {
                            media_type: "anime",
                            title,
                            url,
                            confidence: 0.8,
                        });
                    }
                }
            }
        }

        // Check for TV show mentions
        for pattern in &self.tv_show_patterns {
            for cap in pattern.captures_iter(content) {
                if let Some(title_match) = cap.get(1) {
                    let title = self.clean_title(title_match.as_str());
                    if !title.is_empty() && title.len() > 2 && !found_titles.contains_key(&title) {
                        found_titles.insert(title.clone(), "tv_show");

                        let url =
                            self.find_nearby_url(content, title_match.start(), title_match.end());

                        recommendations.push(MediaRecommendation {
                            media_type: "tv_show",
                            title,
                            url,
                            confidence: 0.7,
                        });
                    }
                }
            }
        }

        // Check for game mentions
        for pattern in &self.game_patterns {
            for cap in pattern.captures_iter(content) {
                if let Some(title_match) = cap.get(1) {
                    let title = self.clean_title(title_match.as_str());
                    if !title.is_empty() && title.len() > 2 && !found_titles.contains_key(&title) {
                        found_titles.insert(title.clone(), "game");

                        let url =
                            self.find_nearby_url(content, title_match.start(), title_match.end());

                        recommendations.push(MediaRecommendation {
                            media_type: "game",
                            title,
                            url,
                            confidence: 0.7,
                        });
                    }
                }
            }
        }

        // Check for YouTube videos
        for cap in self.youtube_pattern.captures_iter(content) {
            if let Some(video_id) = cap.get(1) {
                let url = format!("https://youtube.com/watch?v={}", video_id.as_str());
                let title = format!("YouTube: {}", video_id.as_str());

                if !found_titles.contains_key(&title) {
                    found_titles.insert(title.clone(), "youtube");
                    recommendations.push(MediaRecommendation {
                        media_type: "youtube",
                        title,
                        url: Some(url),
                        confidence: 1.0,
                    });
                }
            }
        }

        // Look for streaming service mentions with context
        let streaming_services = [
            (
                "Netflix",
                vec!["watching on Netflix", "check out .+ on Netflix"],
            ),
            (
                "Crunchyroll",
                vec!["on Crunchyroll", "watching .+ on Crunchyroll"],
            ),
            (
                "Steam",
                vec!["on Steam", "get it on Steam", "playing .+ on Steam"],
            ),
        ];

        for (service, patterns) in &streaming_services {
            for pattern_str in patterns {
                if let Ok(pattern) = Regex::new(&format!("(?i){}", pattern_str)) {
                    if pattern.is_match(content) {
                        // Increase confidence for already found items
                        for rec in &mut recommendations {
                            if content.contains(&rec.title) && content.contains(service) {
                                rec.confidence = (rec.confidence + 0.1).min(1.0);
                            }
                        }
                    }
                }
            }
        }

        recommendations
    }

    fn clean_title(&self, title: &str) -> String {
        let cleaned = title
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric() && c != ' ')
            .replace("  ", " ");

        // Remove common words at the end
        let stop_words = [
            "the", "a", "an", "and", "or", "but", "is", "was", "are", "were",
        ];
        let words: Vec<&str> = cleaned.split_whitespace().collect();

        if words.len() > 1 && stop_words.contains(&words.last().unwrap().to_lowercase().as_str()) {
            words[..words.len() - 1].join(" ")
        } else {
            cleaned
        }
    }

    fn find_nearby_url(&self, content: &str, start: usize, end: usize) -> Option<String> {
        // Look for URLs within 50 characters before or after the title mention
        let search_start = start.saturating_sub(50);
        let search_end = (end + 50).min(content.len());
        let search_area = &content[search_start..search_end];

        if let Some(url_match) = self.url_pattern.find(search_area) {
            Some(url_match.as_str().to_string())
        } else {
            None
        }
    }
}
