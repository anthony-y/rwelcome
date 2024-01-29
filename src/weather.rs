use reqwest;
use serde::{Serialize, Deserialize};
use std::env;

#[derive(Serialize, Deserialize, Debug)]
pub struct LocationInfo {
    pub name: String,
    pub region: String,
    pub country: String,
    pub lat: f64,
    pub lon: f64,
    pub tz_id: String,
    pub localtime_epoch: i64,
    pub localtime: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConditionInfo {
    pub text: String,
    pub icon: String,
    pub code: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentWeatherInfo {
    pub last_updated_epoch: i64,
    pub last_updated: String,
    pub temp_c: f64,
    pub temp_f: f64,
    pub is_day: u8,
    pub condition: ConditionInfo,
    pub wind_mph: f64,
    pub wind_kph: f64,
    pub wind_degree: u16,
    pub wind_dir: String,
    pub pressure_mb: f64,
    pub pressure_in: f64,
    pub precip_mm: f64,
    pub precip_in: f64,
    pub humidity: u8,
    pub cloud: u8,
    pub feelslike_c: f64,
    pub feelslike_f: f64,
    pub vis_km: f64,
    pub vis_miles: f64,
    pub uv: f64,
    pub gust_mph: f64,
    pub gust_kph: f64,
}

#[derive(Deserialize, Debug)]
pub struct WeatherResponse {
    pub location: LocationInfo,
    pub current: CurrentWeatherInfo,
}

pub async fn acquire(key: String) -> reqwest::Result<WeatherResponse> {
    let location = env::var("RWELCOME_WEATHER_LOCATION")
                          .unwrap_or_else(|_| "Brighton".to_string());
    let url = format!("https://api.weatherapi.com/v1/current.json?key={key}&q={location}&aqi=no");
    let res = reqwest::get(url).await?;
    let weather_res: WeatherResponse = res.json().await?;
    Ok(weather_res)
}
