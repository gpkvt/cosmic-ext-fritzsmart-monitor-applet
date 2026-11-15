use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct CommonsAttribution {
    pub title: String,
    pub artist: String,
    pub license: String,
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
    pub image_attribution: Option<CommonsAttribution>,
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

pub async fn fetch_weather_forecast(location: String) -> Result<WeatherForecast, String> {
    let location = location.trim();
    if location.is_empty() {
        return Err("Weather location is empty".into());
    }
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
            return Err(format!("Location lookup returned HTTP {}", response.status()));
        }
        let payload: serde_json::Value = response.json().await
            .map_err(|error| format!("Invalid location response: {error}"))?;
        let result = payload["results"]
            .as_array()
            .and_then(|results| results.first())
            .ok_or_else(|| format!("Could not find weather location '{location}'"))?;
        (
            result["latitude"].as_f64().ok_or("Location has no latitude")?,
            result["longitude"].as_f64().ok_or("Location has no longitude")?,
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
            ("forecast_days", "7".to_string()),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .await
        .map_err(|error| format!("7-day forecast request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Forecast provider returned HTTP {}", response.status()));
    }
    let payload: serde_json::Value = response.json().await
        .map_err(|error| format!("Invalid forecast response: {error}"))?;
    let daily = &payload["daily"];
    let dates = daily["time"].as_array().ok_or("Forecast has no dates")?;
    let codes = daily["weather_code"].as_array().ok_or("Forecast has no weather codes")?;
    let minimums = daily["temperature_2m_min"].as_array()
        .ok_or("Forecast has no minimum temperatures")?;
    let maximums = daily["temperature_2m_max"].as_array()
        .ok_or("Forecast has no maximum temperatures")?;
    let precipitation = daily["precipitation_probability_max"].as_array();
    let wind = daily["wind_speed_10m_max"].as_array();
    let mut days = Vec::with_capacity(7);
    for index in 0..dates.len().min(7) {
        let date = dates[index].as_str().ok_or("Invalid forecast date")?;
        let code = codes[index].as_u64().ok_or("Invalid forecast weather code")?
            .min(255) as u8;
        days.push(WeatherForecastDay {
            date: date.to_string(),
            weather_code: code,
            description: weather_code_description(code).to_string(),
            min_temperature_c: minimums[index].as_f64()
                .ok_or("Invalid minimum temperature")?,
            max_temperature_c: maximums[index].as_f64()
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
    let mut request = client.get("https://api.openweathermap.org/data/2.5/weather")
        .query(&[("appid", api_key.as_str()), ("units", "metric")]);
    if let Some((latitude, longitude)) = parse_coordinates(location) {
        request = request.query(&[("lat", latitude.to_string()), ("lon", longitude.to_string())]);
    } else {
        request = request.query(&[("q", location.to_string())]);
    }
    let response = request.send().await
        .map_err(|error| format!("OpenWeatherMap request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("OpenWeatherMap returned HTTP {status}"));
    }
    let payload: serde_json::Value = response.json().await
        .map_err(|error| format!("Invalid OpenWeatherMap response: {error}"))?;
    let icon_code = payload["weather"][0]["icon"].as_str()
        .ok_or_else(|| "OpenWeatherMap response has no weather icon".to_string())?;
    let description = payload["weather"][0]["description"].as_str()
        .unwrap_or("Current conditions").to_string();
    let main = &payload["main"];
    let temperature_c = main["temp"].as_f64()
        .ok_or_else(|| "OpenWeatherMap response has no temperature".to_string())?;
    let feels_like_c = main["feels_like"].as_f64().unwrap_or(temperature_c);
    let humidity_percent = main["humidity"].as_u64().unwrap_or(0).min(100) as u8;
    let pressure_hpa = main["pressure"].as_u64().unwrap_or(0) as u32;
    let wind_speed_mps = payload["wind"]["speed"].as_f64().unwrap_or(0.0);
    let wind_direction_degrees = payload["wind"]["deg"].as_u64().map(|value| (value % 360) as u16);
    let cloudiness_percent = payload["clouds"]["all"].as_u64().unwrap_or(0).min(100) as u8;
    let visibility_m = payload["visibility"].as_u64().unwrap_or(0) as u32;
    let sunrise_unix = payload["sys"]["sunrise"].as_i64().unwrap_or(0);
    let sunset_unix = payload["sys"]["sunset"].as_i64().unwrap_or(0);
    let location_name = payload["name"].as_str().unwrap_or(location).to_string();
    // Commons photos are selected by the observed condition. Image failures are
    // deliberately non-fatal: weather readings remain available without a photo.
    let (background_image, image_attribution) = fetch_commons_background(&client, icon_code).await;
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
        image_attribution,
    })
}

fn commons_search_term(icon_code: &str) -> &'static str {
    match icon_code.get(..2).unwrap_or("") {
        "01" => "clear sunny sky",
        "02" | "03" | "04" => "cloudy sky",
        "09" | "10" => "rain clouds",
        "11" => "thunderstorm clouds",
        "13" => "snowfall",
        "50" => "fog landscape",
        _ => "weather sky",
    }
}

async fn fetch_commons_background(client: &Client, icon_code: &str) -> (Option<Vec<u8>>, Option<CommonsAttribution>) {
    let search = commons_search_term(icon_code);
    let response = client.get("https://commons.wikimedia.org/w/api.php")
        .query(&[
            ("action", "query"), ("generator", "search"),
            ("gsrsearch", search), ("gsrnamespace", "6"), ("gsrlimit", "1"),
            ("prop", "imageinfo"), ("iiprop", "url|extmetadata"),
            ("iiurlwidth", "1000"), ("format", "json"),
        ])
        .send().await;
    let Ok(response) = response else { return (None, None); };
    let response = response.json::<serde_json::Value>().await;
    let Ok(response) = response else { return (None, None); };
    let Some(page) = response["query"]["pages"].as_object().and_then(|pages| pages.values().next()) else {
        return (None, None);
    };
    let info = &page["imageinfo"][0];
    let Some(image_url) = info["thumburl"].as_str() else { return (None, None); };
    let image_response = client.get(image_url).send().await;
    let Ok(image_response) = image_response else { return (None, None); };
    let image = image_response.bytes().await;
    let Ok(image) = image else { return (None, None); };
    // Blur the decorative photo before displaying it behind weather details,
    // keeping the text legible without hiding the condition-matched image.
    let image = blur_background_image(image.to_vec()).unwrap_or_else(|| image.to_vec());
    let artist = info["extmetadata"]["Artist"]["value"].as_str().unwrap_or("Unknown author");
    let license = info["extmetadata"]["LicenseShortName"]["value"].as_str().unwrap_or("See file page");
    let title = page["title"].as_str().unwrap_or("Wikimedia Commons photograph");
    let page_url = info["descriptionurl"].as_str().unwrap_or("https://commons.wikimedia.org/");
    let attribution = CommonsAttribution {
        title: title.to_string(),
        artist: strip_html(artist),
        license: license.to_string(),
        page_url: commons_file_page_url(page_url),
    };
    (Some(image), Some(attribution))
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

fn blur_background_image(bytes: Vec<u8>) -> Option<Vec<u8>> {
    let decoded = image::load_from_memory(&bytes).ok()?;
    let blurred = decoded.blur(6.0).to_rgb8();
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(blurred)
        .write_to(&mut output, image::ImageFormat::Jpeg)
        .ok()?;
    Some(output.into_inner())
}

fn strip_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut inside_tag = false;
    for character in value.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {},
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_coordinates(location: &str) -> Option<(f64, f64)> {
    let (latitude, longitude) = location.split_once(',')?;
    let latitude = latitude.trim().parse::<f64>().ok()?;
    let longitude = longitude.trim().parse::<f64>().ok()?;
    if !latitude.is_finite() || !longitude.is_finite()
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
                icon_code: code.into(), description: String::new(), temperature_c: 0.0,
                feels_like_c: 0.0, humidity_percent: 0, pressure_hpa: 0,
                wind_speed_mps: 0.0, wind_direction_degrees: None,
                cloudiness_percent: 0, visibility_m: 0, sunrise_unix: 0,
                sunset_unix: 0, location_name: String::new(),
                background_image: None, image_attribution: None,
            };
            assert_eq!(weather.icon_name(), expected);
        }
    }

    #[test]
    fn commons_photo_search_matches_current_weather_condition() {
        assert_eq!(commons_search_term("01d"), "clear sunny sky");
        assert_eq!(commons_search_term("10n"), "rain clouds");
        assert_eq!(commons_search_term("11d"), "thunderstorm clouds");
        assert_eq!(commons_search_term("13n"), "snowfall");
        assert_eq!(commons_search_term("50d"), "fog landscape");
    }

    #[test]
    fn blurs_background_photos_without_changing_their_dimensions() {
        let source = image::DynamicImage::ImageRgb8(
            image::RgbImage::from_pixel(16, 12, image::Rgb([120, 80, 40])),
        );
        let mut encoded = std::io::Cursor::new(Vec::new());
        source.write_to(&mut encoded, image::ImageFormat::Png).unwrap();

        let blurred = blur_background_image(encoded.into_inner()).unwrap();
        let decoded = image::load_from_memory(&blurred).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 12));
    }

    #[test]
    fn parses_valid_coordinates_and_rejects_out_of_range_values() {
        assert_eq!(parse_coordinates("52.37, 4.90"), Some((52.37, 4.90)));
        assert_eq!(parse_coordinates("91,4"), None);
        assert_eq!(parse_coordinates("52,181"), None);
        assert_eq!(parse_coordinates("Amsterdam"), None);
    }
}
