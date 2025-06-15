-- Create table for GIPHY search terms
CREATE TABLE IF NOT EXISTS giphy_search_terms (
    id INT PRIMARY KEY AUTO_INCREMENT,
    search_term VARCHAR(255) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    priority INT DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_active_priority (is_active, priority DESC)
);

-- Create table for GIPHY cache
CREATE TABLE IF NOT EXISTS giphy_cache (
    id INT PRIMARY KEY AUTO_INCREMENT,
    search_term VARCHAR(255) NOT NULL,
    gif_id VARCHAR(255) NOT NULL,
    gif_url VARCHAR(1024) NOT NULL,
    gif_title VARCHAR(512),
    gif_rating VARCHAR(10),
    cached_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_used DATETIME DEFAULT CURRENT_TIMESTAMP,
    use_count INT DEFAULT 0,
    file_size_bytes BIGINT,
    width INT,
    height INT,
    UNIQUE KEY unique_gif (search_term, gif_id),
    INDEX idx_search_term (search_term),
    INDEX idx_cached_at (cached_at),
    INDEX idx_last_used (last_used)
);

-- Insert default search terms for Destiny memes
INSERT INTO giphy_search_terms (search_term, priority) VALUES
    ('Destiny memes', 100),
    ('Destiny 2 memes', 100),
    ('Destiny game memes', 90),
    ('Destiny 2 funny', 80),
    ('Destiny guardian memes', 70);