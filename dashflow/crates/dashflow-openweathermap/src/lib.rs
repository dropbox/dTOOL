// NOTE: needless_pass_by_value was removed - all pass-by-value is intentional
// String ownership is transferred to struct fields

//! `OpenWeatherMap` API integration for `DashFlow` Rust.
//!
//! This crate provides tools for accessing `OpenWeatherMap` weather data APIs.
//!
//! # Features
//!
//! - Current weather data for any location
//! - Weather forecasts (optional)
//! - Multiple query formats: city name, coordinates, zip code
//! - Temperature units: Metric (Celsius), Imperial (Fahrenheit), Kelvin
//! - Free tier available (60 calls/minute, 1M calls/month)
//!
//! # API Documentation
//!
//! - API: [OpenWeatherMap Current Weather API](https://openweathermap.org/current)
//! - Pricing: [OpenWeatherMap Pricing](https://openweathermap.org/price)
//! - Free tier: 60 calls/minute, 1,000,000 calls/month
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_openweathermap::OpenWeatherMapTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let tool = OpenWeatherMapTool::new("your-api-key".to_string())
//!         .with_units("metric");
//!
//!     let result = tool._call(ToolInput::String("London".to_string())).await?;
//!     println!("{}", result);
//!     Ok(())
//! }
//! ```

use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::http_client::{json_with_limit, DEFAULT_RESPONSE_SIZE_LIMIT};
use dashflow::core::tools::{Tool, ToolInput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn extract_query_from_input(input: ToolInput) -> Result<String, dashflow::core::Error> {
    match input {
        ToolInput::String(s) => Ok(s),
        ToolInput::Structured(value) => value
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                dashflow::core::Error::tool_error("Missing 'query' field in structured input")
            })
            .map(std::string::ToString::to_string),
    }
}

/// `OpenWeatherMap` tool for retrieving current weather data.
///
/// This tool uses the `OpenWeatherMap` Current Weather Data API to get real-time
/// weather information for any location worldwide.
///
/// # API Details
///
/// - **Endpoint**: `https://api.openweathermap.org/data/2.5/weather`
/// - **Authentication**: API key required (free tier available)
/// - **Rate Limits**: 60 calls/minute (free tier)
/// - **Query Formats**:
///   - City name: "London", "London,UK", "London,GB"
///   - Coordinates: "lat=51.5074&lon=-0.1278"
///   - Zip code: "zip=94040,US"
/// - **Units**:
///   - `metric`: Celsius, meters/sec (default)
///   - `imperial`: Fahrenheit, miles/hour
///   - `standard`: Kelvin, meters/sec
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_openweathermap::OpenWeatherMapTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let tool = OpenWeatherMapTool::new("your-api-key".to_string())
///     .with_units("metric");
///
/// // Query by city name
/// let weather = tool._call(ToolInput::String("Paris,FR".to_string())).await?;
/// println!("{}", weather);
///
/// // Query by coordinates
/// let weather = tool._call(ToolInput::String("lat=48.8566&lon=2.3522".to_string())).await?;
/// println!("{}", weather);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct OpenWeatherMapTool {
    /// `OpenWeatherMap` API key
    api_key: String,
    /// Temperature units: "metric", "imperial", or "standard" (default: "metric")
    units: String,
    /// Request timeout in seconds (default: 30)
    timeout: u64,
    /// HTTP client
    client: reqwest::Client,
}

// Custom Debug implementation to prevent API key exposure in logs
impl std::fmt::Debug for OpenWeatherMapTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenWeatherMapTool")
            .field("api_key", &"[REDACTED]")
            .field("units", &self.units)
            .field("timeout", &self.timeout)
            .field("client", &"reqwest::Client")
            .finish()
    }
}

impl OpenWeatherMapTool {
    /// Creates a new `OpenWeatherMap` tool with an API key.
    ///
    /// # Arguments
    ///
    /// * `api_key` - `OpenWeatherMap` API key (get one at <https://openweathermap.org/api>)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_openweathermap::OpenWeatherMapTool;
    ///
    /// let tool = OpenWeatherMapTool::new("your-api-key".to_string());
    /// ```
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            units: "metric".to_string(),
            timeout: 30,
            client: create_http_client(),
        }
    }

    /// Sets the temperature units for weather data.
    ///
    /// # Arguments
    ///
    /// * `units` - One of "metric" (Celsius), "imperial" (Fahrenheit), or "standard" (Kelvin)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_openweathermap::OpenWeatherMapTool;
    ///
    /// let tool = OpenWeatherMapTool::new("key".to_string())
    ///     .with_units("imperial"); // Use Fahrenheit
    /// ```
    #[must_use]
    pub fn with_units(mut self, units: &str) -> Self {
        self.units = units.to_string();
        self
    }

    /// Sets the request timeout in seconds.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Timeout in seconds (default: 30)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_openweathermap::OpenWeatherMapTool;
    ///
    /// let tool = OpenWeatherMapTool::new("key".to_string())
    ///     .with_timeout(60);
    /// ```
    #[must_use]
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    /// Formats weather data into a human-readable string.
    fn format_weather(&self, data: &WeatherResponse) -> String {
        let temp_unit = match self.units.as_str() {
            "metric" => "°C",
            "imperial" => "°F",
            _ => "K",
        };
        let speed_unit = match self.units.as_str() {
            "imperial" => "mph",
            _ => "m/s",
        };

        let weather_desc = data.weather.first().map_or_else(
            || "Unknown".to_string(),
            |w| format!("{} ({})", w.main, w.description),
        );

        format!(
            "Weather in {}, {}:\n\
             - Condition: {}\n\
             - Temperature: {}{} (feels like: {}{})\n\
             - Humidity: {}%\n\
             - Wind Speed: {}{}\n\
             - Pressure: {} hPa\n\
             - Visibility: {} meters\n\
             - Cloudiness: {}%",
            data.name,
            data.sys.country,
            weather_desc,
            data.main.temp,
            temp_unit,
            data.main.feels_like,
            temp_unit,
            data.main.humidity,
            data.wind.speed,
            speed_unit,
            data.main.pressure,
            data.visibility.unwrap_or(0),
            data.clouds.all
        )
    }

    /// Parses the query string to build API parameters.
    ///
    /// Supports:
    /// - City name: "London" or "London,UK"
    /// - Coordinates: "lat=51.5074&lon=-0.1278"
    /// - Zip code: "zip=94040,US"
    fn parse_query(&self, query: &str) -> HashMap<String, String> {
        let query = query.trim();
        let mut params = HashMap::new();

        // Check if query contains lat/lon parameters
        if query.contains("lat=") && query.contains("lon=") {
            for part in query.split('&') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if let Some((key, value)) = part.split_once('=') {
                    let key = key.trim();
                    if key.is_empty() {
                        continue;
                    }
                    params.insert(key.to_string(), value.trim().to_string());
                }
            }
        }
        // Check if query is a zip code
        else if query.starts_with("zip=") {
            if let Some(zip_value) = query.strip_prefix("zip=") {
                params.insert("zip".to_string(), zip_value.trim().to_string());
            }
        }
        // Otherwise treat as city name
        else {
            params.insert("q".to_string(), query.to_string());
        }

        params.insert("appid".to_string(), self.api_key.clone());
        params.insert("units".to_string(), self.units.clone());

        params
    }
}

#[async_trait::async_trait]
impl Tool for OpenWeatherMapTool {
    fn name(&self) -> &'static str {
        "openweathermap"
    }

    fn description(&self) -> &'static str {
        "Get current weather data for any location worldwide. \
         Input should be a city name (e.g., 'London', 'Paris,FR'), \
         coordinates (e.g., 'lat=51.5074&lon=-0.1278'), \
         or zip code (e.g., 'zip=94040,US'). \
         Returns current temperature, conditions, humidity, wind speed, and more."
    }

    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Location to get weather for. Can be city name (London), coordinates (lat=51&lon=-0.1), or zip code (zip=94040,US)"
                }
            },
            "required": ["query"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String, dashflow::core::Error> {
        let query = extract_query_from_input(input)?;

        let params = self.parse_query(&query);
        let url = "https://api.openweathermap.org/data/2.5/weather";

        let response = self
            .client
            .get(url)
            .query(&params)
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("OpenWeatherMap API request failed: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(dashflow::core::Error::tool_error(format!(
                "OpenWeatherMap API error ({status}): {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let weather_data: WeatherResponse = json_with_limit(response, DEFAULT_RESPONSE_SIZE_LIMIT)
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!(
                    "Failed to parse OpenWeatherMap response: {e}"
                ))
            })?;

        Ok(self.format_weather(&weather_data))
    }
}

/// `OpenWeatherMap` API response structure for current weather data.
#[derive(Debug, Deserialize, Serialize)]
struct WeatherResponse {
    /// Weather condition information
    weather: Vec<WeatherCondition>,
    /// Main weather parameters (temperature, pressure, humidity)
    main: MainWeather,
    /// Wind information
    wind: Wind,
    /// Cloudiness
    clouds: Clouds,
    /// Visibility in meters (optional, may be missing in some responses)
    visibility: Option<i32>,
    /// System information (country, sunrise, sunset)
    sys: Sys,
    /// City name
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct WeatherCondition {
    /// Weather condition id
    id: i32,
    /// Group of weather parameters (Rain, Snow, Extreme, etc.)
    main: String,
    /// Weather condition description
    description: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct MainWeather {
    /// Temperature
    temp: f64,
    /// Temperature feels like
    feels_like: f64,
    /// Minimum temperature
    temp_min: f64,
    /// Maximum temperature
    temp_max: f64,
    /// Atmospheric pressure in hPa
    pressure: i32,
    /// Humidity percentage
    humidity: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Wind {
    /// Wind speed (m/s or mph depending on units)
    speed: f64,
    /// Wind direction in degrees
    #[serde(default)]
    deg: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Clouds {
    /// Cloudiness percentage
    all: i32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Sys {
    /// Country code (e.g., "GB", "US")
    country: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tool() {
        let tool = OpenWeatherMapTool::new("test-api-key".to_string());
        assert_eq!(tool.name(), "openweathermap");
        assert_eq!(tool.api_key, "test-api-key");
        assert_eq!(tool.units, "metric");
        assert_eq!(tool.timeout, 30);
    }

    #[test]
    fn test_builder_pattern() {
        let tool = OpenWeatherMapTool::new("key".to_string())
            .with_units("imperial")
            .with_timeout(60);

        assert_eq!(tool.units, "imperial");
        assert_eq!(tool.timeout, 60);
    }

    #[test]
    fn test_name() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        assert_eq!(tool.name(), "openweathermap");
    }

    #[test]
    fn test_description() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let desc = tool.description();
        assert!(desc.contains("weather"));
        assert!(desc.contains("location"));
        assert!(desc.contains("city name"));
    }

    #[test]
    fn test_args_schema() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["properties"]["query"]["type"], "string");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("query")));
    }

    #[test]
    fn test_parse_query_city() {
        let tool = OpenWeatherMapTool::new("test-key".to_string());
        let params = tool.parse_query("London");

        assert_eq!(params.get("q"), Some(&"London".to_string()));
        assert_eq!(params.get("appid"), Some(&"test-key".to_string()));
        assert_eq!(params.get("units"), Some(&"metric".to_string()));
    }

    #[test]
    fn test_parse_query_city_with_country() {
        let tool = OpenWeatherMapTool::new("test-key".to_string());
        let params = tool.parse_query("London,UK");

        assert_eq!(params.get("q"), Some(&"London,UK".to_string()));
    }

    #[test]
    fn test_parse_query_coordinates() {
        let tool = OpenWeatherMapTool::new("test-key".to_string());
        let params = tool.parse_query("lat=51.5074&lon=-0.1278");

        assert_eq!(params.get("lat"), Some(&"51.5074".to_string()));
        assert_eq!(params.get("lon"), Some(&"-0.1278".to_string()));
        assert!(!params.contains_key("q"));
    }

    #[test]
    fn test_parse_query_zip() {
        let tool = OpenWeatherMapTool::new("test-key".to_string());
        let params = tool.parse_query("zip=94040,US");

        assert_eq!(params.get("zip"), Some(&"94040,US".to_string()));
        assert!(!params.contains_key("q"));
    }

    #[test]
    fn test_format_weather() {
        let tool = OpenWeatherMapTool::new("test-key".to_string());
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 20.5,
                feels_like: 19.8,
                temp_min: 18.0,
                temp_max: 22.0,
                pressure: 1013,
                humidity: 65,
            },
            wind: Wind {
                speed: 3.5,
                deg: 180,
            },
            clouds: Clouds { all: 10 },
            visibility: Some(10000),
            sys: Sys {
                country: "GB".to_string(),
            },
            name: "London".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("London, GB"));
        assert!(formatted.contains("Clear (clear sky)"));
        assert!(formatted.contains("20.5°C"));
        assert!(formatted.contains("feels like: 19.8°C"));
        assert!(formatted.contains("Humidity: 65%"));
        assert!(formatted.contains("Wind Speed: 3.5m/s"));
        assert!(formatted.contains("Pressure: 1013 hPa"));
    }

    #[test]
    fn test_format_weather_imperial() {
        let tool = OpenWeatherMapTool::new("test-key".to_string()).with_units("imperial");
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 68.0,
                feels_like: 67.0,
                temp_min: 65.0,
                temp_max: 70.0,
                pressure: 1013,
                humidity: 65,
            },
            wind: Wind {
                speed: 7.8,
                deg: 180,
            },
            clouds: Clouds { all: 10 },
            visibility: Some(10000),
            sys: Sys {
                country: "US".to_string(),
            },
            name: "San Francisco".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("68°F"));
        assert!(formatted.contains("7.8mph"));
    }

    // Integration tests - run with real API keys when available
    #[tokio::test]
    #[ignore = "requires OPENWEATHERMAP_API_KEY"]
    async fn test_call_city_name() {
        let api_key =
            std::env::var("OPENWEATHERMAP_API_KEY").expect("OPENWEATHERMAP_API_KEY must be set");

        let tool = OpenWeatherMapTool::new(api_key);
        let result = tool._call(ToolInput::String("London,UK".to_string())).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("London"));
        assert!(output.contains("Temperature"));
        assert!(output.contains("Humidity"));
    }

    #[tokio::test]
    #[ignore = "requires OPENWEATHERMAP_API_KEY"]
    async fn test_call_coordinates() {
        let api_key =
            std::env::var("OPENWEATHERMAP_API_KEY").expect("OPENWEATHERMAP_API_KEY must be set");

        let tool = OpenWeatherMapTool::new(api_key);
        let result = tool
            ._call(ToolInput::String("lat=51.5074&lon=-0.1278".to_string()))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Temperature"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_call_invalid_api_key() {
        let tool = OpenWeatherMapTool::new("invalid-key".to_string());
        let result = tool._call(ToolInput::String("London".to_string())).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("401") || err.to_string().contains("API error"));
    }

    // ========================================================================
    // Comprehensive Tests - Edge Cases & Data Structures
    // ========================================================================

    #[test]
    fn test_debug_redacts_api_key() {
        let tool = OpenWeatherMapTool::new("secret-api-key-12345".to_string());
        let debug_output = format!("{:?}", tool);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("secret-api-key-12345"));
    }

    #[test]
    fn test_clone() {
        let tool = OpenWeatherMapTool::new("key".to_string())
            .with_units("imperial")
            .with_timeout(120);
        let cloned = tool.clone();

        assert_eq!(cloned.api_key, "key");
        assert_eq!(cloned.units, "imperial");
        assert_eq!(cloned.timeout, 120);
    }

    #[test]
    fn test_format_weather_kelvin_units() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("standard");
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 293.15, // ~20°C in Kelvin
                feels_like: 292.0,
                temp_min: 290.0,
                temp_max: 295.0,
                pressure: 1013,
                humidity: 50,
            },
            wind: Wind { speed: 5.0, deg: 90 },
            clouds: Clouds { all: 0 },
            visibility: Some(10000),
            sys: Sys {
                country: "JP".to_string(),
            },
            name: "Tokyo".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("293.15K"));
        assert!(formatted.contains("m/s")); // Kelvin uses m/s not mph
    }

    #[test]
    fn test_format_weather_missing_visibility() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 741,
                main: "Fog".to_string(),
                description: "fog".to_string(),
            }],
            main: MainWeather {
                temp: 5.0,
                feels_like: 3.0,
                temp_min: 4.0,
                temp_max: 6.0,
                pressure: 1020,
                humidity: 95,
            },
            wind: Wind { speed: 1.0, deg: 0 },
            clouds: Clouds { all: 100 },
            visibility: None, // Missing visibility
            sys: Sys {
                country: "UK".to_string(),
            },
            name: "Edinburgh".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("Visibility: 0 meters")); // Should use default
        assert!(formatted.contains("Fog (fog)"));
    }

    #[test]
    fn test_format_weather_empty_weather_array() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let response = WeatherResponse {
            weather: vec![], // Empty weather conditions
            main: MainWeather {
                temp: 15.0,
                feels_like: 14.0,
                temp_min: 13.0,
                temp_max: 16.0,
                pressure: 1010,
                humidity: 60,
            },
            wind: Wind { speed: 2.5, deg: 180 },
            clouds: Clouds { all: 50 },
            visibility: Some(8000),
            sys: Sys {
                country: "DE".to_string(),
            },
            name: "Berlin".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("Condition: Unknown")); // Default when no weather
    }

    #[test]
    fn test_format_weather_multiple_conditions() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let response = WeatherResponse {
            weather: vec![
                WeatherCondition {
                    id: 500,
                    main: "Rain".to_string(),
                    description: "light rain".to_string(),
                },
                WeatherCondition {
                    id: 701,
                    main: "Mist".to_string(),
                    description: "mist".to_string(),
                },
            ],
            main: MainWeather {
                temp: 10.0,
                feels_like: 8.0,
                temp_min: 9.0,
                temp_max: 11.0,
                pressure: 1005,
                humidity: 90,
            },
            wind: Wind {
                speed: 4.0,
                deg: 270,
            },
            clouds: Clouds { all: 80 },
            visibility: Some(5000),
            sys: Sys {
                country: "IE".to_string(),
            },
            name: "Dublin".to_string(),
        };

        let formatted = tool.format_weather(&response);
        // Should use first condition only
        assert!(formatted.contains("Rain (light rain)"));
        assert!(!formatted.contains("Mist"));
    }

    #[test]
    fn test_parse_query_empty_string() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("");
        assert_eq!(params.get("q"), Some(&String::new()));
        assert!(params.contains_key("appid"));
    }

    #[test]
    fn test_parse_query_unicode_city() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("東京");
        assert_eq!(params.get("q"), Some(&"東京".to_string()));
    }

    #[test]
    fn test_parse_query_city_with_spaces() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("New York");
        assert_eq!(params.get("q"), Some(&"New York".to_string()));
    }

    #[test]
    fn test_parse_query_coordinates_negative() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=-33.8688&lon=151.2093"); // Sydney
        assert_eq!(params.get("lat"), Some(&"-33.8688".to_string()));
        assert_eq!(params.get("lon"), Some(&"151.2093".to_string()));
    }

    #[test]
    fn test_parse_query_coordinates_with_extra_params() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=40.7128&lon=-74.0060&extra=ignored");
        assert_eq!(params.get("lat"), Some(&"40.7128".to_string()));
        assert_eq!(params.get("lon"), Some(&"-74.0060".to_string()));
        assert_eq!(params.get("extra"), Some(&"ignored".to_string()));
    }

    #[test]
    fn test_parse_query_malformed_lat_only() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        // Only lat, no lon - should be treated as city name
        let params = tool.parse_query("lat=51.5074");
        assert_eq!(params.get("q"), Some(&"lat=51.5074".to_string()));
        assert!(!params.contains_key("lat")); // Should NOT parse as coordinate
    }

    #[test]
    fn test_parse_query_zip_international() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("zip=SW1A,GB");
        assert_eq!(params.get("zip"), Some(&"SW1A,GB".to_string()));
    }

    #[test]
    fn test_parse_query_zip_numeric_only() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("zip=10001");
        assert_eq!(params.get("zip"), Some(&"10001".to_string()));
    }

    #[test]
    fn test_with_units_metric() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("metric");
        assert_eq!(tool.units, "metric");
    }

    #[test]
    fn test_with_units_standard() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("standard");
        assert_eq!(tool.units, "standard");
    }

    #[test]
    fn test_with_timeout_zero() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_timeout(0);
        assert_eq!(tool.timeout, 0);
    }

    #[test]
    fn test_with_timeout_large() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_timeout(3600);
        assert_eq!(tool.timeout, 3600);
    }

    #[test]
    fn test_weather_response_serialization() {
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 20.0,
                feels_like: 19.0,
                temp_min: 18.0,
                temp_max: 22.0,
                pressure: 1015,
                humidity: 55,
            },
            wind: Wind {
                speed: 3.0,
                deg: 180,
            },
            clouds: Clouds { all: 5 },
            visibility: Some(10000),
            sys: Sys {
                country: "FR".to_string(),
            },
            name: "Paris".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: WeatherResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, "Paris");
        assert_eq!(parsed.main.temp, 20.0);
        assert_eq!(parsed.weather[0].main, "Clear");
    }

    #[test]
    fn test_weather_response_deserialization_minimal() {
        let json = r#"{
            "weather": [{"id": 500, "main": "Rain", "description": "light rain"}],
            "main": {"temp": 12.5, "feels_like": 11.0, "temp_min": 10.0, "temp_max": 14.0, "pressure": 1010, "humidity": 80},
            "wind": {"speed": 5.0},
            "clouds": {"all": 75},
            "sys": {"country": "NL"},
            "name": "Amsterdam"
        }"#;

        let parsed: WeatherResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.name, "Amsterdam");
        assert_eq!(parsed.wind.deg, 0); // Default when missing
        assert!(parsed.visibility.is_none());
    }

    #[test]
    fn test_weather_condition_debug() {
        let condition = WeatherCondition {
            id: 200,
            main: "Thunderstorm".to_string(),
            description: "thunderstorm with light rain".to_string(),
        };
        let debug_output = format!("{:?}", condition);
        assert!(debug_output.contains("Thunderstorm"));
        assert!(debug_output.contains("200"));
    }

    #[test]
    fn test_main_weather_extreme_values() {
        let main = MainWeather {
            temp: -89.2, // Coldest recorded on Earth
            feels_like: -100.0,
            temp_min: -90.0,
            temp_max: -80.0,
            pressure: 870, // Low pressure
            humidity: 100,
        };

        let json = serde_json::to_string(&main).unwrap();
        let parsed: MainWeather = serde_json::from_str(&json).unwrap();
        assert!((parsed.temp - (-89.2)).abs() < 0.01);
    }

    #[test]
    fn test_format_weather_high_values() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("imperial");
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 134.0, // Death Valley record
                feels_like: 140.0,
                temp_min: 130.0,
                temp_max: 136.0,
                pressure: 1000,
                humidity: 5, // Very dry
            },
            wind: Wind {
                speed: 50.0, // Strong wind
                deg: 45,
            },
            clouds: Clouds { all: 0 },
            visibility: Some(100000), // Very clear
            sys: Sys {
                country: "US".to_string(),
            },
            name: "Death Valley".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("134°F"));
        assert!(formatted.contains("Humidity: 5%"));
        assert!(formatted.contains("50mph"));
    }

    #[test]
    fn test_format_weather_special_characters_city() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear sky".to_string(),
            }],
            main: MainWeather {
                temp: 25.0,
                feels_like: 24.0,
                temp_min: 23.0,
                temp_max: 27.0,
                pressure: 1012,
                humidity: 40,
            },
            wind: Wind {
                speed: 2.0,
                deg: 120,
            },
            clouds: Clouds { all: 10 },
            visibility: Some(10000),
            sys: Sys {
                country: "MX".to_string(),
            },
            name: "México City".to_string(), // Accented character
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("México City, MX"));
    }

    #[test]
    fn test_wind_default_deg() {
        let json = r#"{"speed": 10.0}"#;
        let wind: Wind = serde_json::from_str(json).unwrap();
        assert_eq!(wind.speed, 10.0);
        assert_eq!(wind.deg, 0); // Default value
    }

    #[test]
    fn test_wind_with_deg() {
        let json = r#"{"speed": 10.0, "deg": 270}"#;
        let wind: Wind = serde_json::from_str(json).unwrap();
        assert_eq!(wind.speed, 10.0);
        assert_eq!(wind.deg, 270);
    }

    #[test]
    fn test_builder_chain() {
        let tool = OpenWeatherMapTool::new("api-key".to_string())
            .with_units("metric")
            .with_timeout(45)
            .with_units("imperial") // Override previous
            .with_timeout(90); // Override previous

        assert_eq!(tool.units, "imperial");
        assert_eq!(tool.timeout, 90);
    }

    #[test]
    fn test_default_http_client_creation() {
        // Test that create_http_client doesn't panic
        let _client = create_http_client();
        // If we get here, client was created successfully
    }

    // ========================================================================
    // ToolInput Extraction (No-Network)
    // ========================================================================

    #[test]
    fn test_extract_query_from_string_input() {
        let query =
            extract_query_from_input(ToolInput::String("London".to_string())).expect("ok");
        assert_eq!(query, "London");
    }

    #[test]
    fn test_extract_query_from_structured_input_ok() {
        let query = extract_query_from_input(ToolInput::Structured(serde_json::json!({
            "query": "Paris,FR"
        })))
        .expect("ok");
        assert_eq!(query, "Paris,FR");
    }

    #[test]
    fn test_extract_query_from_structured_input_missing_query() {
        let err = extract_query_from_input(ToolInput::Structured(serde_json::json!({
            "city": "London"
        })))
        .expect_err("missing query must error");
        assert!(err.to_string().contains("Missing 'query' field"));
    }

    #[test]
    fn test_extract_query_from_structured_input_query_wrong_type() {
        let err = extract_query_from_input(ToolInput::Structured(serde_json::json!({
            "query": 123
        })))
        .expect_err("non-string query must error");
        assert!(err.to_string().contains("Missing 'query' field"));
    }

    // ========================================================================
    // parse_query() Robustness (Trimming & Coordinates)
    // ========================================================================

    #[test]
    fn test_parse_query_trims_city() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("  London  ");
        assert_eq!(params.get("q"), Some(&"London".to_string()));
    }

    #[test]
    fn test_parse_query_trims_coordinates_parts() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query(" lat=51.5074 & lon=-0.1278 ");
        assert_eq!(params.get("lat"), Some(&"51.5074".to_string()));
        assert_eq!(params.get("lon"), Some(&"-0.1278".to_string()));
        assert!(!params.contains_key("q"));
    }

    #[test]
    fn test_parse_query_coordinates_reversed_order() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lon=-0.1278&lat=51.5074");
        assert_eq!(params.get("lat"), Some(&"51.5074".to_string()));
        assert_eq!(params.get("lon"), Some(&"-0.1278".to_string()));
        assert!(!params.contains_key("q"));
    }

    #[test]
    fn test_parse_query_coordinates_last_value_wins() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=1&lon=2&lat=3");
        assert_eq!(params.get("lat"), Some(&"3".to_string()));
        assert_eq!(params.get("lon"), Some(&"2".to_string()));
    }

    #[test]
    fn test_parse_query_trims_zip_value() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("zip= 94040,US ");
        assert_eq!(params.get("zip"), Some(&"94040,US".to_string()));
    }

    #[test]
    fn test_parse_query_zip_empty_value() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("zip=");
        assert_eq!(params.get("zip"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_query_uses_tool_units() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("imperial");
        let params = tool.parse_query("London");
        assert_eq!(params.get("units"), Some(&"imperial".to_string()));
    }

    #[test]
    fn test_parse_query_includes_api_key() {
        let tool = OpenWeatherMapTool::new("secret".to_string());
        let params = tool.parse_query("London");
        assert_eq!(params.get("appid"), Some(&"secret".to_string()));
    }

    // ========================================================================
    // format_weather() Output Guarantees
    // ========================================================================

    #[test]
    fn test_format_weather_always_includes_cloudiness_and_visibility() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 801,
                main: "Clouds".to_string(),
                description: "few clouds".to_string(),
            }],
            main: MainWeather {
                temp: 1.0,
                feels_like: -1.0,
                temp_min: 0.0,
                temp_max: 2.0,
                pressure: 1000,
                humidity: 90,
            },
            wind: Wind { speed: 0.5, deg: 0 },
            clouds: Clouds { all: 42 },
            visibility: None,
            sys: Sys {
                country: "SE".to_string(),
            },
            name: "Stockholm".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("Cloudiness: 42%"));
        assert!(formatted.contains("Visibility: 0 meters"));
    }

    #[test]
    fn test_format_weather_unknown_units_falls_back_to_kelvin_and_ms() {
        let tool = OpenWeatherMapTool::new("key".to_string()).with_units("unknown");
        let response = WeatherResponse {
            weather: vec![WeatherCondition {
                id: 800,
                main: "Clear".to_string(),
                description: "clear".to_string(),
            }],
            main: MainWeather {
                temp: 300.0,
                feels_like: 299.0,
                temp_min: 298.0,
                temp_max: 301.0,
                pressure: 1013,
                humidity: 10,
            },
            wind: Wind { speed: 1.0, deg: 10 },
            clouds: Clouds { all: 0 },
            visibility: Some(1),
            sys: Sys {
                country: "ZZ".to_string(),
            },
            name: "Nowhere".to_string(),
        };

        let formatted = tool.format_weather(&response);
        assert!(formatted.contains("300K"));
        assert!(formatted.contains("1m/s"));
    }

    // ========================================================================
    // Serde Required Fields
    // ========================================================================

    fn minimal_weather_response_json() -> serde_json::Value {
        serde_json::json!({
            "weather": [{"id": 800, "main": "Clear", "description": "clear sky"}],
            "main": {
                "temp": 20.5,
                "feels_like": 19.8,
                "temp_min": 18.0,
                "temp_max": 22.0,
                "pressure": 1013,
                "humidity": 65
            },
            "wind": {"speed": 3.5},
            "clouds": {"all": 10},
            "sys": {"country": "GB"},
            "name": "London"
        })
    }

    macro_rules! assert_top_level_missing_field_fails {
        ($test_name:ident, $field:expr) => {
            #[test]
            fn $test_name() {
                let mut json = minimal_weather_response_json();
                json.as_object_mut()
                    .expect("object")
                    .remove($field)
                    .expect("field present");
                let err = serde_json::from_value::<WeatherResponse>(json)
                    .expect_err("missing required field must error");
                assert!(err.to_string().contains($field));
            }
        };
    }

    assert_top_level_missing_field_fails!(test_weather_response_missing_weather_fails, "weather");
    assert_top_level_missing_field_fails!(test_weather_response_missing_main_fails, "main");
    assert_top_level_missing_field_fails!(test_weather_response_missing_wind_fails, "wind");
    assert_top_level_missing_field_fails!(test_weather_response_missing_clouds_fails, "clouds");
    assert_top_level_missing_field_fails!(test_weather_response_missing_sys_fails, "sys");
    assert_top_level_missing_field_fails!(test_weather_response_missing_name_fails, "name");

    macro_rules! assert_weather_condition_missing_field_fails {
        ($test_name:ident, $field:expr) => {
            #[test]
            fn $test_name() {
                let mut json = minimal_weather_response_json();
                let weather = json
                    .get_mut("weather")
                    .and_then(serde_json::Value::as_array_mut)
                    .expect("weather array");
                let condition = weather
                    .get_mut(0)
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("weather[0] object");
                condition.remove($field).expect("field present");
                let err = serde_json::from_value::<WeatherResponse>(json)
                    .expect_err("missing required field must error");
                assert!(err.to_string().contains($field));
            }
        };
    }

    assert_weather_condition_missing_field_fails!(test_weather_condition_missing_id_fails, "id");
    assert_weather_condition_missing_field_fails!(test_weather_condition_missing_main_fails, "main");
    assert_weather_condition_missing_field_fails!(
        test_weather_condition_missing_description_fails,
        "description"
    );

    macro_rules! assert_main_weather_missing_field_fails {
        ($test_name:ident, $field:expr) => {
            #[test]
            fn $test_name() {
                let mut json = minimal_weather_response_json();
                let main = json
                    .get_mut("main")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("main object");
                main.remove($field).expect("field present");
                let err = serde_json::from_value::<WeatherResponse>(json)
                    .expect_err("missing required field must error");
                assert!(err.to_string().contains($field));
            }
        };
    }

    assert_main_weather_missing_field_fails!(test_main_weather_missing_temp_fails, "temp");
    assert_main_weather_missing_field_fails!(test_main_weather_missing_feels_like_fails, "feels_like");
    assert_main_weather_missing_field_fails!(test_main_weather_missing_temp_min_fails, "temp_min");
    assert_main_weather_missing_field_fails!(test_main_weather_missing_temp_max_fails, "temp_max");
    assert_main_weather_missing_field_fails!(test_main_weather_missing_pressure_fails, "pressure");
    assert_main_weather_missing_field_fails!(test_main_weather_missing_humidity_fails, "humidity");

    #[test]
    fn test_wind_missing_speed_fails() {
        let mut json = minimal_weather_response_json();
        let wind = json
            .get_mut("wind")
            .and_then(serde_json::Value::as_object_mut)
            .expect("wind object");
        wind.remove("speed").expect("speed present");
        let err = serde_json::from_value::<WeatherResponse>(json)
            .expect_err("missing required field must error");
        assert!(err.to_string().contains("speed"));
    }

    #[test]
    fn test_wind_deg_null_fails() {
        let mut json = minimal_weather_response_json();
        let wind = json
            .get_mut("wind")
            .and_then(serde_json::Value::as_object_mut)
            .expect("wind object");
        wind.insert("deg".to_string(), serde_json::Value::Null);
        let err = serde_json::from_value::<WeatherResponse>(json)
            .expect_err("null deg must error");
        let msg = err.to_string();
        assert!(msg.contains("deg") || msg.contains("invalid type") || msg.contains("i32"));
    }

    #[test]
    fn test_clouds_missing_all_fails() {
        let mut json = minimal_weather_response_json();
        let clouds = json
            .get_mut("clouds")
            .and_then(serde_json::Value::as_object_mut)
            .expect("clouds object");
        clouds.remove("all").expect("all present");
        let err = serde_json::from_value::<WeatherResponse>(json)
            .expect_err("missing required field must error");
        assert!(err.to_string().contains("all"));
    }

    #[test]
    fn test_sys_missing_country_fails() {
        let mut json = minimal_weather_response_json();
        let sys = json
            .get_mut("sys")
            .and_then(serde_json::Value::as_object_mut)
            .expect("sys object");
        sys.remove("country").expect("country present");
        let err = serde_json::from_value::<WeatherResponse>(json)
            .expect_err("missing required field must error");
        assert!(err.to_string().contains("country"));
    }

    // ========================================================================
    // parse_query() Edge Cases
    // ========================================================================

    #[test]
    fn test_parse_query_coordinates_ignores_empty_parts() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=1&&lon=2&");
        assert_eq!(params.get("lat"), Some(&"1".to_string()));
        assert_eq!(params.get("lon"), Some(&"2".to_string()));
    }

    #[test]
    fn test_parse_query_coordinates_ignores_parts_without_equal_sign() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=1&lon=2&nonsense");
        assert_eq!(params.get("lat"), Some(&"1".to_string()));
        assert_eq!(params.get("lon"), Some(&"2".to_string()));
        assert!(!params.contains_key("nonsense"));
    }

    #[test]
    fn test_parse_query_coordinates_ignores_empty_key() {
        let tool = OpenWeatherMapTool::new("key".to_string());
        let params = tool.parse_query("lat=1&=ignored&lon=2");
        assert_eq!(params.get("lat"), Some(&"1".to_string()));
        assert_eq!(params.get("lon"), Some(&"2".to_string()));
        assert!(!params.contains_key(""));
    }
}
