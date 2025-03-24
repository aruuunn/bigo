use serde::Deserialize;

#[derive(Deserialize)]
pub struct LocationDTO {
    pub id: String,
    pub seismic_activity: f64,
    pub temperature_c: f64,
    pub radiation_level: f64,
    pub location_id: String,
}