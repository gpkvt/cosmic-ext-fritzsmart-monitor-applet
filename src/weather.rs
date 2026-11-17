use reqwest::Client;
use std::sync::LazyLock;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct CommonsAttribution {
    pub title: String,
    pub artist: String,
    pub license: String,
    pub license_url: Option<String>,
    pub page_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherData {
    pub icon_code: String,
    pub description: String,
    pub temperature_c: f64,
    pub feels_like_c: f64,
    pub humidity_percent: u8,
    pub pressure_hpa: u32,
    pub wind_speed_mps: f64,
    pub wind_direction_degrees: Option<u16>,
    pub cloudiness_percent: u8,
    pub visibility_m: u32,
    pub sunrise_unix: i64,
    pub sunset_unix: i64,
    pub location_name: String,
    pub background_image: Option<Vec<u8>>,
    pub background_images: Vec<Vec<u8>>,
    pub slideshow_frames: Vec<Vec<u8>>,
    pub slideshow_sequence: Vec<usize>,
    pub slideshow_frame_sources: Vec<Vec<usize>>,
    pub image_attribution: Option<CommonsAttribution>,
    pub image_attributions: Vec<CommonsAttribution>,
}

impl WeatherData {
    pub fn icon_name(&self) -> &'static str {
        match self.icon_code.as_str() {
            "01n" => "weather-clear-night-symbolic",
            "02n" => "weather-few-clouds-night-symbolic",
            "01d" => "weather-clear-symbolic",
            "02d" => "weather-few-clouds-symbolic",
            "03d" | "03n" | "04d" | "04n" => "weather-overcast-symbolic",
            "09d" | "09n" | "10d" | "10n" => "weather-showers-symbolic",
            "11d" | "11n" => "weather-storm-symbolic",
            "13d" | "13n" => "weather-snow-symbolic",
            "50d" | "50n" => "weather-fog-symbolic",
            _ => "weather-clear-symbolic",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherForecast {
    pub days: Vec<WeatherForecastDay>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherForecastDay {
    pub date: String,
    pub weather_code: u8,
    pub description: String,
    pub min_temperature_c: f64,
    pub max_temperature_c: f64,
    pub precipitation_probability_percent: u8,
    pub wind_speed_kmh: f64,
}

impl WeatherForecastDay {
    pub fn icon_name(&self) -> &'static str {
        match self.weather_code {
            0 => "weather-clear-symbolic",
            1 | 2 => "weather-few-clouds-symbolic",
            3 => "weather-overcast-symbolic",
            45 | 48 => "weather-fog-symbolic",
            51..=57 | 61..=67 | 80..=82 => "weather-showers-symbolic",
            71..=77 | 85..=86 => "weather-snow-symbolic",
            95 | 96 | 99 => "weather-storm-symbolic",
            _ => "weather-overcast-symbolic",
        }
    }
}

fn weather_code_description(code: u8) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 => "Light drizzle",
        53 => "Drizzle",
        55 | 56 | 57 => "Dense drizzle",
        61 => "Light rain",
        63 => "Rain",
        65 | 66 | 67 => "Heavy rain",
        71 => "Light snow",
        73 => "Snow",
        75 | 77 => "Heavy snow",
        80 => "Rain showers",
        81 | 82 => "Heavy showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Variable conditions",
    }
}

pub async fn fetch_weather_forecast(
    location: String,
    requested_days: u8,
) -> Result<WeatherForecast, String> {
    let location = location.trim();
    if location.is_empty() {
        return Err("Weather location is empty".into());
    }
    let forecast_days: u8 = if requested_days == 14 { 14 } else { 7 };
    let client = Client::builder()
        .user_agent("FRITZ-Power-Monitor/0.2.6")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("Forecast client error: {error}"))?;

    let (latitude, longitude) = if let Some(coordinates) = parse_coordinates(location) {
        coordinates
    } else {
        let response = client
            .get("https://geocoding-api.open-meteo.com/v1/search")
            .query(&[
                ("name", location),
                ("count", "1"),
                ("language", "en"),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(|error| format!("Location lookup failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Location lookup returned HTTP {}",
                response.status()
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|error| format!("Invalid location response: {error}"))?;
        let result = payload["results"]
            .as_array()
            .and_then(|results| results.first())
            .ok_or_else(|| format!("Could not find weather location '{location}'"))?;
        (
            result["latitude"]
                .as_f64()
                .ok_or("Location has no latitude")?,
            result["longitude"]
                .as_f64()
                .ok_or("Location has no longitude")?,
        )
    };

    let response = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", latitude.to_string()),
            ("longitude", longitude.to_string()),
            (
                "daily",
                "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,wind_speed_10m_max".to_string(),
            ),
            ("forecast_days", forecast_days.to_string()),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .await
        .map_err(|error| format!("{forecast_days}-day forecast request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Forecast provider returned HTTP {}",
            response.status()
        ));
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("Invalid forecast response: {error}"))?;
    let daily = &payload["daily"];
    let dates = daily["time"].as_array().ok_or("Forecast has no dates")?;
    let codes = daily["weather_code"]
        .as_array()
        .ok_or("Forecast has no weather codes")?;
    let minimums = daily["temperature_2m_min"]
        .as_array()
        .ok_or("Forecast has no minimum temperatures")?;
    let maximums = daily["temperature_2m_max"]
        .as_array()
        .ok_or("Forecast has no maximum temperatures")?;
    let precipitation = daily["precipitation_probability_max"].as_array();
    let wind = daily["wind_speed_10m_max"].as_array();
    let mut days = Vec::with_capacity(usize::from(forecast_days));
    for index in 0..dates.len().min(usize::from(forecast_days)) {
        let date = dates[index].as_str().ok_or("Invalid forecast date")?;
        let code = codes[index]
            .as_u64()
            .ok_or("Invalid forecast weather code")?
            .min(255) as u8;
        days.push(WeatherForecastDay {
            date: date.to_string(),
            weather_code: code,
            description: weather_code_description(code).to_string(),
            min_temperature_c: minimums[index]
                .as_f64()
                .ok_or("Invalid minimum temperature")?,
            max_temperature_c: maximums[index]
                .as_f64()
                .ok_or("Invalid maximum temperature")?,
            precipitation_probability_percent: precipitation
                .and_then(|values| values.get(index))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                .min(100) as u8,
            wind_speed_kmh: wind
                .and_then(|values| values.get(index))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0),
        });
    }
    if days.is_empty() {
        return Err("Forecast provider returned no forecast days".into());
    }
    Ok(WeatherForecast { days })
}

pub async fn fetch_weather(api_key: String, location: String) -> Result<WeatherData, String> {
    if api_key.trim().is_empty() {
        return Err("OpenWeatherMap API key is empty".into());
    }
    let location = location.trim();
    if location.is_empty() {
        return Err("Weather location is empty".into());
    }

    let client = Client::builder()
        .user_agent("FRITZ-Power-Monitor/0.2.1 (https://github.com/gpkvt/cosmic-solar-applet)")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("Weather client error: {error}"))?;
    let mut request = client
        .get("https://api.openweathermap.org/data/2.5/weather")
        .query(&[("appid", api_key.as_str()), ("units", "metric")]);
    if let Some((latitude, longitude)) = parse_coordinates(location) {
        request = request.query(&[
            ("lat", latitude.to_string()),
            ("lon", longitude.to_string()),
        ]);
    } else {
        request = request.query(&[("q", location.to_string())]);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("OpenWeatherMap request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("OpenWeatherMap returned HTTP {status}"));
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("Invalid OpenWeatherMap response: {error}"))?;
    let icon_code = payload["weather"][0]["icon"]
        .as_str()
        .ok_or_else(|| "OpenWeatherMap response has no weather icon".to_string())?;
    let description = payload["weather"][0]["description"]
        .as_str()
        .unwrap_or("Current conditions")
        .to_string();
    let main = &payload["main"];
    let temperature_c = main["temp"]
        .as_f64()
        .ok_or_else(|| "OpenWeatherMap response has no temperature".to_string())?;
    let feels_like_c = main["feels_like"].as_f64().unwrap_or(temperature_c);
    let humidity_percent = main["humidity"].as_u64().unwrap_or(0).min(100) as u8;
    let pressure_hpa = main["pressure"].as_u64().unwrap_or(0) as u32;
    let wind_speed_mps = payload["wind"]["speed"].as_f64().unwrap_or(0.0);
    let wind_direction_degrees = payload["wind"]["deg"]
        .as_u64()
        .map(|value| (value % 360) as u16);
    let cloudiness_percent = payload["clouds"]["all"].as_u64().unwrap_or(0).min(100) as u8;
    let visibility_m = payload["visibility"].as_u64().unwrap_or(0) as u32;
    let sunrise_unix = payload["sys"]["sunrise"].as_i64().unwrap_or(0);
    let sunset_unix = payload["sys"]["sunset"].as_i64().unwrap_or(0);
    let location_name = payload["name"].as_str().unwrap_or(location).to_string();
    // Select photos using both the observed condition and OpenWeatherMap's
    // local day/night indicator. Image failures remain non-fatal.
    let (mut background_images, image_attributions) = fetch_commons_background(&client, icon_code).await;
    let image_attribution = image_attributions.first().cloned();
    // At night, never substitute an unrelated image when Commons cannot
    // provide a credible match. Use a locally generated star field instead.
    if icon_code.ends_with('n') && background_images.is_empty() {
        if let Some(stars) = generate_starry_night_background() {
            background_images.push(stars);
        }
    }
    let background_image = background_images.first().cloned();
    // The applet builds its slideshow with the user's configured display duration.
    let slideshow_frames = Vec::new();
    let slideshow_sequence = Vec::new();
    Ok(WeatherData {
        icon_code: icon_code.to_string(),
        description,
        temperature_c,
        feels_like_c,
        humidity_percent,
        pressure_hpa,
        wind_speed_mps,
        wind_direction_degrees,
        cloudiness_percent,
        visibility_m,
        sunrise_unix,
        sunset_unix,
        location_name,
        background_image,
        background_images,
        slideshow_frames,
        slideshow_sequence,
        image_attribution,
        image_attributions,
        slideshow_frame_sources: Vec::new(),
    })
}

fn commons_search_terms(icon_code: &str) -> &'static [&'static str] {
    let is_night = icon_code.ends_with('n');
    match (icon_code.get(..2).unwrap_or(""), is_night) {
        ("01", true) => &[
            "clear starry night sky landscape",
            "clear night sky over city skyline",
            "moon stars night mountain landscape",
        ],
        ("01", false) => &[
            "sunny blue sky landscape",
            "bright sunshine over mountain landscape",
            "sunlit blue sky countryside",
        ],
        ("02" | "03" | "04", true) => &[
            "cloudy night sky moon city",
            "dramatic clouds over city at night",
            "moon behind clouds night landscape",
        ],
        ("02" | "03" | "04", false) => &[
            "cloudy sky landscape daytime",
            "dramatic overcast clouds mountain",
            "cloud formations over countryside",
        ],
        ("09" | "10", true) => &[
            "rainy night city street reflections",
            "rain falling at night urban street",
            "night rain illuminated street",
        ],
        ("09" | "10", false) => &[
            "rain shower daytime landscape",
            "rain falling over city daytime",
            "rain clouds over green countryside",
        ],
        ("11", true) => &[
            "lightning thunderstorm night city",
            "lightning storm over landscape at night",
            "dramatic thunderstorm night sky",
        ],
        ("11", false) => &[
            "thunderstorm lightning dark clouds daytime",
            "storm clouds over landscape",
            "approaching thunderstorm countryside",
        ],
        ("13", true) => &[
            "snowfall at night village lights",
            "snowy night forest landscape",
            "winter night snow covered street",
        ],
        ("13", false) => &[
            "snowfall daytime forest",
            "snow covered mountain winter landscape",
            "snowy village in daylight",
        ],
        ("50", true) => &[
            "foggy night street lights",
            "misty night city landscape",
            "fog over forest at night",
        ],
        ("50", false) => &[
            "morning fog forest landscape",
            "misty countryside morning",
            "foggy city street daylight",
        ],
        (_, true) => &["night weather landscape", "night sky city lights"],
        _ => &["weather sky landscape daytime", "dramatic sky landscape"],
    }
}

fn commons_search_term(icon_code: &str, variant: usize) -> &'static str {
    let terms = commons_search_terms(icon_code);
    terms[variant % terms.len()]
}

fn commons_title_matches_condition(icon_code: &str, title: &str) -> bool {
    let normalized = title.to_ascii_lowercase();
    let words: Vec<&str> = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    // Exact tokens only: prefixes and generic landscape/night words are not
    // evidence that a file actually depicts the requested weather.
    let has = |terms: &[&str]| words.iter().any(|word| terms.contains(word));

    // Commons search can return editorial assets whose titles only mention
    // weather. Reject these before considering condition keywords.
    let non_photo_terms = [
        "book", "novel", "film", "movie", "cover", "poster", "illustration",
        "drawing", "painting", "cartoon", "comic", "manga", "album", "song",
        "soundtrack", "game", "logo", "advertisement", "advert", "theatre",
        "theater", "fiction", "character", "sculpture", "statue", "map",
        "diagram", "chart", "screenshot",
    ];
    if has(&non_photo_terms) {
        return false;
    }

    let rain = ["rain", "rains", "raining", "rainy", "shower", "showers", "drizzle", "drizzling", "precipitation"];
    let storm = ["storm", "storms", "stormy", "thunder", "thunderstorm", "lightning"];
    let snow = ["snow", "snowfall", "snowy", "blizzard"];
    let fog = ["fog", "foggy", "mist", "misty", "haze", "hazy"];
    let clouds = ["cloud", "clouds", "cloudy", "overcast", "cumulus", "stratus"];

    match icon_code.get(..2).unwrap_or("") {
        "01" if icon_code.ends_with('n') => {
            has(&["clear", "star", "stars", "starry", "moon", "moonlight"])
                && !has(&rain) && !has(&storm) && !has(&snow) && !has(&fog)
        }
        "01" => {
            (has(&["sun", "sunny", "sunshine", "sunlight", "cloudless"]) || has(&["blue", "sky"]) && normalized.contains("blue sky") || has(&["clear"]) && normalized.contains("clear sky"))
                && !has(&rain) && !has(&storm) && !has(&snow) && !has(&fog)
        }
        "02" | "03" | "04" => {
            has(&clouds) && !has(&rain) && !has(&storm) && !has(&snow) && !has(&fog)
        }
        "09" | "10" => has(&rain) && !has(&storm) && !has(&snow),
        "11" => has(&storm),
        "13" => has(&snow),
        "50" => has(&fog),
        _ => false,
    }
}

async fn fetch_commons_background(
    client: &Client,
    icon_code: &str,
) -> (Vec<Vec<u8>>, Vec<CommonsAttribution>) {
    let mut images = Vec::new();
    let mut seen_titles = std::collections::HashSet::new();
    let mut attributions = Vec::new();
    // Search every condition-specific phrase. Avoid rotating through arbitrary
    // result offsets, which admitted unrelated files into the slideshow.
    for variant in 0..commons_search_terms(icon_code).len() {
        let search = commons_search_term(icon_code, variant);
        let response = client.get("https://commons.wikimedia.org/w/api.php")
            .query(&[
                ("action", "query"), ("generator", "search"),
                ("gsrsearch", search), ("gsrnamespace", "6"),
                ("gsrlimit", "20"),
                ("prop", "imageinfo|categories"), ("iiprop", "url|extmetadata"),
                ("cllimit", "20"), ("clshow", "!hidden"),
                ("iiurlwidth", "1200"), ("format", "json"),
            ]).send().await;
        let Ok(response) = response else { continue };
        if !response.status().is_success() { continue; }
        let Ok(response) = response.json::<serde_json::Value>().await else { continue };
        let Some(pages) = response["query"]["pages"].as_object() else { continue };
        for page in pages.values() {
            if images.len() >= 5 { break; }
            let title = page["title"].as_str().unwrap_or("");
            // Use the file title itself as the relevance signal. Categories
            // are often broad or inherited and can incorrectly admit unrelated
            // editorial material into a weather-photo slideshow.
            if !commons_title_matches_condition(icon_code, title) {
                continue;
            }
            if !seen_titles.insert(title.to_string()) { continue; }
            let info = &page["imageinfo"][0];
            let Some(image_url) = info["thumburl"].as_str().or_else(|| info["url"].as_str()) else { continue };
            let Ok(response) = client.get(image_url).send().await else { continue };
            if !response.status().is_success() { continue; }
            let Ok(bytes) = response.bytes().await else { continue };
            // Keep the original photograph intact. The weather card adds a
            // transparent black chessboard overlay at display time.
            let image = bytes.to_vec();
            let metadata = &info["extmetadata"];
            let artist = metadata["Artist"]["value"].as_str()
                .or_else(|| metadata["Credit"]["value"].as_str())
                .filter(|value| !strip_html(value).is_empty())
                .map(strip_html)
                .unwrap_or_else(|| "Author not provided by Wikimedia Commons".to_string());
            let license = metadata["LicenseShortName"]["value"].as_str()
                .or_else(|| metadata["UsageTerms"]["value"].as_str())
                .map(strip_html)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "License details on Wikimedia Commons file page".to_string());
            let license_url = metadata["LicenseUrl"]["value"].as_str()
                .filter(|url| url.starts_with("https://") && !url.chars().any(char::is_control))
                .map(str::to_string);
            let title = page["title"].as_str().unwrap_or("Wikimedia Commons photograph");
            let page_url = info["descriptionurl"].as_str().unwrap_or("https://commons.wikimedia.org/");
            attributions.push(CommonsAttribution {
                title: title.to_string(), artist, license, license_url,
                page_url: commons_file_page_url(page_url),
            });
            images.push(image);
        }
        if images.len() >= 5 { break; }
    }
    (images, attributions)
}

fn commons_file_page_url(value: &str) -> String {
    if value.starts_with("https://commons.wikimedia.org/wiki/File:")
        && !value.chars().any(char::is_control)
    {
        value.to_string()
    } else {
        "https://commons.wikimedia.org/".to_string()
    }
}


/// Creates a deterministic, locally generated starry-night backdrop. This is
/// deliberately independent of network services and external image metadata.
/// Transparent star layer composited over every nighttime weather image.
/// The fixed seed keeps stars stable while the weather card redraws.
pub fn starry_night_overlay() -> Option<Vec<u8>> {
    static OVERLAY: LazyLock<Option<Vec<u8>>> = LazyLock::new(|| {
        let width = 1200u32;
        let height = 520u32;
        let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
        let mut seed = 0x51a7_2026_u32;
        let mut random = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed
        };

        for _ in 0..190 {
            let x = random() % width;
            let y = random() % (height * 3 / 4);
            let brightness = 175u8.saturating_add((random() % 81) as u8);
            let alpha = 105u8.saturating_add((random() % 125) as u8);
            let radius = if brightness > 230 && random() % 7 == 0 { 1i32 } else { 0i32 };
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dy * dy > radius * radius {
                        continue;
                    }
                    let px = x as i32 + dx;
                    let py = y as i32 + dy;
                    if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                        image.put_pixel(
                            px as u32,
                            py as u32,
                            image::Rgba([brightness.saturating_sub(8), brightness, 255, alpha]),
                        );
                    }
                }
            }
        }

        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .ok()?;
        Some(output.into_inner())
    });
    OVERLAY.clone()
}

fn generate_starry_night_background() -> Option<Vec<u8>> {
    let width = 1200u32;
    let height = 520u32;
    let mut image = image::RgbImage::from_fn(width, height, |_, y| {
        let t = y as f32 / height as f32;
        let blend = (t * 0.72).clamp(0.0, 1.0);
        image::Rgb([
            (8.0 + 8.0 * blend) as u8,
            (13.0 + 10.0 * blend) as u8,
            (34.0 + 18.0 * blend) as u8,
        ])
    });

    // Fixed-seed generator keeps the fallback visually stable between refreshes.
    let mut seed = 0x5eed_c0de_u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for _ in 0..230 {
        let x = random() % width;
        let y = random() % (height * 3 / 4);
        let brightness = 150u8.saturating_add((random() % 106) as u8);
        let radius = if brightness > 225 && random() % 5 == 0 { 2i32 } else { 1i32 };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius { continue; }
                let px = x as i32 + dx;
                let py = y as i32 + dy;
                if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                    image.put_pixel(px as u32, py as u32, image::Rgb([
                        brightness.saturating_sub(12),
                        brightness.saturating_sub(5),
                        brightness,
                    ]));
                }
            }
        }
    }

    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut output, image::ImageFormat::Jpeg)
        .ok()?;
    Some(output.into_inner())
}

/// A black chessboard texture for photo-backed weather cards. Every other
/// square is black at 50% opacity; the alternating squares are fully transparent.
pub fn chessboard_background_overlay() -> Option<Vec<u8>> {
    static OVERLAY: LazyLock<Option<Vec<u8>>> = LazyLock::new(|| {
        let width = 1200u32;
        let height = 520u32;
        let square_size = 16u32;
        let image = image::RgbaImage::from_fn(width, height, |x, y| {
            if ((x / square_size) + (y / square_size)) % 2 == 0 {
                image::Rgba([0, 0, 0, 128])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .ok()?;
        Some(output.into_inner())
    });
    OVERLAY.clone()
}

pub fn build_weather_slideshow_frames(
    images: &[Vec<u8>],
    display_seconds: u16,
) -> (Vec<Vec<u8>>, Vec<usize>, Vec<Vec<usize>>) {
    use image::{DynamicImage, ImageBuffer, Rgba};
    let decoded: Vec<_> = images.iter()
        .filter_map(|bytes| image::load_from_memory(bytes).ok())
        .map(|image| image.to_rgba8())
        .collect();
    if decoded.is_empty() { return (Vec::new(), Vec::new(), Vec::new()); }

    let mut frames = Vec::new();
    let mut sequence = Vec::new();
    let mut frame_sources = Vec::new();
    let hold_ticks = usize::from(display_seconds.clamp(5, 300)) * 4;
    for (index, current) in decoded.iter().enumerate() {
        let mut output = std::io::Cursor::new(Vec::new());
        if DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(current.clone()).to_rgb8())
            .write_to(&mut output, image::ImageFormat::Jpeg).is_ok()
        {
            frames.push(output.into_inner());
            let frame_index = frames.len() - 1;
            sequence.extend(std::iter::repeat(frame_index).take(hold_ticks));
            frame_sources.push(vec![index]);
        }
        if decoded.len() < 2 { continue; }
        let next = &decoded[(index + 1) % decoded.len()];
        if current.dimensions() != next.dimensions() { continue; }
        for step in 1..=3 {
            let amount = step as f32 / 4.0;
            let mut blended: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(current.width(), current.height());
            for (x, y, pixel) in blended.enumerate_pixels_mut() {
                let a = current.get_pixel(x, y).0;
                let b = next.get_pixel(x, y).0;
                *pixel = Rgba(std::array::from_fn(|channel| {
                    (f32::from(a[channel]) * (1.0 - amount) + f32::from(b[channel]) * amount).round() as u8
                }));
            }
            let mut output = std::io::Cursor::new(Vec::new());
            if DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(blended).to_rgb8())
                .write_to(&mut output, image::ImageFormat::Jpeg).is_ok()
            {
                frames.push(output.into_inner());
                sequence.push(frames.len() - 1);
                frame_sources.push(vec![index, (index + 1) % decoded.len()]);
            }
        }
    }
    (frames, sequence, frame_sources)
}

fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut inside_tag = false;
    for character in value.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    let output = output.split_whitespace().collect::<Vec<_>>().join(" ");
    output.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn parse_coordinates(location: &str) -> Option<(f64, f64)> {
    let (latitude, longitude) = location.split_once(',')?;
    let latitude = latitude.trim().parse::<f64>().ok()?;
    let longitude = longitude.trim().parse::<f64>().ok()?;
    if !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=180.0).contains(&longitude)
    {
        return None;
    }
    Some((latitude, longitude))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forecast_weather_codes_map_to_descriptive_conditions_and_icons() {
        for (code, description, icon) in [
            (0, "Clear sky", "weather-clear-symbolic"),
            (2, "Partly cloudy", "weather-few-clouds-symbolic"),
            (3, "Overcast", "weather-overcast-symbolic"),
            (61, "Light rain", "weather-showers-symbolic"),
            (73, "Snow", "weather-snow-symbolic"),
            (95, "Thunderstorm", "weather-storm-symbolic"),
        ] {
            assert_eq!(weather_code_description(code), description);
            let day = WeatherForecastDay {
                date: "2026-11-15".into(),
                weather_code: code,
                description: description.into(),
                min_temperature_c: 0.0,
                max_temperature_c: 1.0,
                precipitation_probability_percent: 0,
                wind_speed_kmh: 0.0,
            };
            assert_eq!(day.icon_name(), icon);
        }
    }

    #[test]
    fn commons_metadata_html_is_rendered_as_readable_text() {
        assert_eq!(
            strip_html("<a href='https://example.org'>A &amp; B</a> &#39;Photo&#39;"),
            "A & B 'Photo'",
        );
    }

    #[test]
    fn commons_attribution_links_only_to_commons_file_pages() {
        assert_eq!(
            commons_file_page_url("https://commons.wikimedia.org/wiki/File:Example.jpg"),
            "https://commons.wikimedia.org/wiki/File:Example.jpg",
        );
        assert_eq!(
            commons_file_page_url("https://example.com/image.jpg"),
            "https://commons.wikimedia.org/",
        );
        assert_eq!(
            commons_file_page_url("javascript:alert(1)"),
            "https://commons.wikimedia.org/",
        );
    }

    #[test]
    fn maps_openweathermap_conditions_to_symbolic_icons() {
        for (code, expected) in [
            ("01d", "weather-clear-symbolic"),
            ("01n", "weather-clear-night-symbolic"),
            ("02d", "weather-few-clouds-symbolic"),
            ("02n", "weather-few-clouds-night-symbolic"),
            ("04n", "weather-overcast-symbolic"),
            ("09d", "weather-showers-symbolic"),
            ("11d", "weather-storm-symbolic"),
            ("13n", "weather-snow-symbolic"),
            ("50d", "weather-fog-symbolic"),
        ] {
            let weather = WeatherData {
                icon_code: code.into(),
                description: String::new(),
                temperature_c: 0.0,
                feels_like_c: 0.0,
                humidity_percent: 0,
                pressure_hpa: 0,
                wind_speed_mps: 0.0,
                wind_direction_degrees: None,
                cloudiness_percent: 0,
                visibility_m: 0,
                sunrise_unix: 0,
                sunset_unix: 0,
                location_name: String::new(),
                background_image: None,
                background_images: Vec::new(),
                slideshow_frames: Vec::new(),
                slideshow_sequence: Vec::new(),
                slideshow_frame_sources: Vec::new(),
                image_attribution: None,
                image_attributions: Vec::new(),
            };
            assert_eq!(weather.icon_name(), expected);
        }
    }

    #[test]
    fn commons_photo_search_matches_condition_and_time_of_day() {
        for (icon, expected) in [
            ("01d", "sunny blue sky landscape"),
            ("01n", "clear starry night sky landscape"),
            ("10n", "rainy night city street reflections"),
            ("10d", "rain shower daytime landscape"),
            ("11n", "lightning thunderstorm night city"),
            ("11d", "thunderstorm lightning dark clouds daytime"),
            ("13n", "snowfall at night village lights"),
            ("50d", "morning fog forest landscape"),
        ] {
            assert_eq!(commons_search_term(icon, 0), expected);
        }
        assert_eq!(commons_search_term("01n", 1), "clear night sky over city skyline");
        assert_eq!(commons_search_term("01n", 2), "moon stars night mountain landscape");
    }

    #[test]
    fn commons_images_must_match_the_reported_weather_condition() {
        for (icon, title, expected) in [
            ("01d", "Sunny blue sky over hills", true),
            ("01n", "Clear starry night sky", true),
            ("01d", "Rain clouds over city", false),
            ("02d", "Cumulus clouds", true),
            ("04d", "Overcast sky", true),
            ("04d", "Beautiful landscape at night", false),
            ("04d", "Rain clouds over the city", false),
            ("02d", "Cloudy with a Chance of Meatballs movie poster", false),
            ("10d", "Heavy rain over a field", true),
            ("10d", "Cloudy sky over the coast", false),
            ("10d", "Thunderstorm with heavy rain", false),
            ("10n", "Rainbow over the city", false),
            ("11d", "Lightning during a thunderstorm", true),
            ("11n", "Snowy mountain at night", false),
            ("13d", "Snowfall in a winter forest", true),
            ("50d", "Misty morning landscape", true),
            ("50n", "Sunny city skyline", false),
        ] {
            assert_eq!(commons_title_matches_condition(icon, title), expected, "{icon}: {title}");
        }
    }

    #[test]
    fn rejects_editorial_assets_that_only_mention_weather() {
        for title in [
            "Cloudy with a Chance of Meatballs book cover",
            "Cloudy Storms movie poster",
            "Thunderstorm illustrated children's book",
            "Rainy Day cartoon character",
            "Snowstorm album cover",
        ] {
            let icon = if title.to_ascii_lowercase().contains("thunderstorm") { "11d" }
                else if title.to_ascii_lowercase().contains("rainy") { "10d" }
                else if title.to_ascii_lowercase().contains("snowstorm") { "13d" }
                else { "04d" };
            assert!(!commons_title_matches_condition(icon, title), "{title}");
        }
        assert!(commons_title_matches_condition("04d", "Cumulus clouds over the coast"));
        assert!(commons_title_matches_condition("11d", "Lightning over the mountains"));
    }

    #[test]
    fn weather_slideshow_contains_hold_and_crossfade_frames() {
        let first = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8, 8, image::Rgb([0, 0, 0]),
        ));
        let second = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8, 8, image::Rgb([255, 255, 255]),
        ));
        let encode = |image: image::DynamicImage| {
            let mut output = std::io::Cursor::new(Vec::new());
            image.write_to(&mut output, image::ImageFormat::Jpeg).unwrap();
            output.into_inner()
        };
        let (frames, sequence, frame_sources) = build_weather_slideshow_frames(&[encode(first), encode(second)], 5);
        assert_eq!(frames.len(), 8);
        assert_eq!(sequence.len(), 46);
        assert_eq!(frame_sources.len(), frames.len());
        assert_eq!(frame_sources[0], vec![0]);
        assert_eq!(frame_sources[1], vec![0, 1]);
        for frame in frames {
            assert!(image::load_from_memory(&frame).is_ok());
        }
    }

    #[test]
    fn generates_a_valid_starry_night_fallback() {
        let bytes = generate_starry_night_background().expect("star field should render");
        let decoded = image::load_from_memory(&bytes).expect("fallback must be a valid image");
        assert_eq!((decoded.width(), decoded.height()), (1200, 520));
    }

    #[test]
    fn chessboard_overlay_alternates_half_opacity_and_transparency() {
        let bytes = chessboard_background_overlay().expect("overlay should render");
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (1200, 520));
        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 128]);
        assert_eq!(decoded.get_pixel(16, 0).0, [0, 0, 0, 0]);
        assert_eq!(decoded.get_pixel(0, 16).0, [0, 0, 0, 0]);
        assert_eq!(decoded.get_pixel(16, 16).0, [0, 0, 0, 128]);
    }

    #[test]
    fn parses_valid_coordinates_and_rejects_out_of_range_values() {
        assert_eq!(parse_coordinates("52.37, 4.90"), Some((52.37, 4.90)));
        assert_eq!(parse_coordinates("91,4"), None);
        assert_eq!(parse_coordinates("52,181"), None);
        assert_eq!(parse_coordinates("Amsterdam"), None);
    }
}
