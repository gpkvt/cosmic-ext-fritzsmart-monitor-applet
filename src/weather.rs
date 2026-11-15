use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherData {
    pub icon_code: String,
    pub description: String,
    pub temperature_c: f64,
}

impl WeatherData {
    pub fn icon_name(&self) -> &'static str {
        match self.icon_code.get(..2).unwrap_or("") {
            "01" => "weather-clear-symbolic",
            "02" => "weather-few-clouds-symbolic",
            "03" | "04" => "weather-overcast-symbolic",
            "09" | "10" => "weather-showers-symbolic",
            "11" => "weather-storm-symbolic",
            "13" => "weather-snow-symbolic",
            "50" => "weather-fog-symbolic",
            _ => "weather-clear-symbolic",
        }
    }
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
    let temperature_c = payload["main"]["temp"].as_f64()
        .ok_or_else(|| "OpenWeatherMap response has no temperature".to_string())?;
    Ok(WeatherData { icon_code: icon_code.to_string(), description, temperature_c })
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
    fn maps_openweathermap_conditions_to_symbolic_icons() {
        for (code, expected) in [
            ("01d", "weather-clear-symbolic"),
            ("01n", "weather-clear-symbolic"),
            ("02d", "weather-few-clouds-symbolic"),
            ("04n", "weather-overcast-symbolic"),
            ("09d", "weather-showers-symbolic"),
            ("11d", "weather-storm-symbolic"),
            ("13n", "weather-snow-symbolic"),
            ("50d", "weather-fog-symbolic"),
        ] {
            let weather = WeatherData { icon_code: code.into(), description: String::new(), temperature_c: 0.0 };
            assert_eq!(weather.icon_name(), expected);
        }
    }

    #[test]
    fn parses_valid_coordinates_and_rejects_out_of_range_values() {
        assert_eq!(parse_coordinates("52.37, 4.90"), Some((52.37, 4.90)));
        assert_eq!(parse_coordinates("91,4"), None);
        assert_eq!(parse_coordinates("52,181"), None);
        assert_eq!(parse_coordinates("Amsterdam"), None);
    }
}
