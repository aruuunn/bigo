use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct LocationStats {
    pub id: String,
    pub seismic_activity: f64,
    pub temperature_c: f64,
    pub radiation_level: f64,
}


#[derive(Deserialize, Serialize, Clone)]
pub struct EnrichedLocationStats {
    pub id: String,
    pub modification_count: i64,
    pub seismic_activity: f64,
    pub temperature_c: f64,
    pub radiation_level: f64,
}

impl EnrichedLocationStats {
    pub fn from(modification_count: i64, locationStats: LocationStats) -> EnrichedLocationStats {
        EnrichedLocationStats {
            modification_count,
            id: locationStats.id,
            seismic_activity: locationStats.seismic_activity,
            temperature_c: locationStats.temperature_c,
            radiation_level: locationStats.radiation_level,
        }
    }
}