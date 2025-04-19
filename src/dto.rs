use serde::{Deserialize, Serialize};
use uuid::{uuid, Uuid};

#[derive(Deserialize, Clone)]
pub struct LocationStats {
    pub id: String,
    pub seismic_activity: f64,
    pub temperature_c: f64,
    pub radiation_level: f64,
}

use std::convert::TryInto;
use std::error::Error;
use std::fmt::{self, write};

// Custom error type for shard operations
#[derive(Debug)]
pub enum ShardError {
    EncodingError(String),
    DecodingError(String),
    InvalidShard(String),
    RpcError(String),
    ChannelError(String),
    ActixError(String),
    NotFoundError(String)
}

impl fmt::Display for ShardError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShardError::EncodingError(msg) => write!(f, "Encoding error: {}", msg),
            ShardError::DecodingError(msg) => write!(f, "Decoding error: {}", msg),
            ShardError::InvalidShard(msg) => write!(f, "Invalid shard: {}", msg),
            ShardError::RpcError(msg) => write!(f, "Rpc Error: {}", msg),
            ShardError::ChannelError(msg) => write!(f, "Channel Error: {}", msg),
            ShardError::ActixError(msg) => write!(f, "Actix Error: {}", msg),
            ShardError::NotFoundError(msg) => write!(f, "Shard Not found Error: {}", msg)
        }
    }
}

impl Error for ShardError {}

// Constants for our encoding
const ID_SIZE: usize = 16; // uuid
const TOTAL_SIZE: usize = ID_SIZE + 8 + 8 + 8 + 8; // id + 4 fields (8 bytes each)
const SHARD_SIZE: usize = TOTAL_SIZE / 4; // Each shard will be exactly this size

#[derive(Deserialize, Serialize, Clone)]
pub struct EnrichedLocationStats {
    pub id: String,
    pub modification_count: i64,
    pub seismic_activity: f64,
    pub temperature_c: f64,
    pub radiation_level: f64,
}

impl EnrichedLocationStats {
    pub fn new(
        id: String,
        modification_count: i64,
        seismic_activity: f64,
        temperature_c: f64,
        radiation_level: f64,
    ) -> Result<Self, ShardError> {
        Ok(Self {
            id,
            modification_count,
            seismic_activity,
            temperature_c,
            radiation_level,
        })
    }
    pub fn from(modification_count: i64, locationStats: LocationStats) -> EnrichedLocationStats {
        EnrichedLocationStats {
            modification_count,
            id: locationStats.id,
            seismic_activity: locationStats.seismic_activity,
            temperature_c: locationStats.temperature_c,
            radiation_level: locationStats.radiation_level,
        }
    }

    // Encode the stats into 4 equal-sized shards
    pub fn to_shards(&self) -> Result<[Vec<u8>; 4], ShardError> {
        // Create a buffer big enough to hold all data
        let mut buffer = vec![0u8; TOTAL_SIZE];

        let uuid = Uuid::parse_str(&self.id).unwrap();
    

        // Encode numeric fields (using little-endian byte order)
        let modification_bytes = self.modification_count.to_le_bytes();
        let seismic_bytes = self.seismic_activity.to_le_bytes();
        let temperature_bytes = self.temperature_c.to_le_bytes();
        let radiation_bytes = self.radiation_level.to_le_bytes();

        buffer[0..ID_SIZE].copy_from_slice(uuid.as_bytes());

        // Copy numeric field bytes into buffer
        let mut offset = ID_SIZE;
        buffer[offset..offset + 8].copy_from_slice(&modification_bytes);
        offset += 8;
        buffer[offset..offset + 8].copy_from_slice(&seismic_bytes);
        offset += 8;
        buffer[offset..offset + 8].copy_from_slice(&temperature_bytes);
        offset += 8;
        buffer[offset..offset + 8].copy_from_slice(&radiation_bytes);

        // Split buffer into 4 equal shards
        let mut shards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for i in 0..4 {
            let start = i * SHARD_SIZE;
            let end = start + SHARD_SIZE;
            shards[i] = buffer[start..end].to_vec();
        }

        Ok(shards)
    }

    // Create a stats object from 4 shards
    pub fn from_shards(shards: [Vec<u8>; 4]) -> Result<Self, ShardError> {
        // Validate shards
        for (i, shard) in shards.iter().enumerate() {
            if shard.len() != SHARD_SIZE {
                return Err(ShardError::InvalidShard(format!(
                    "Shard {} has incorrect size: {} (expected {})",
                    i,
                    shard.len(),
                    SHARD_SIZE
                )));
            }
        }

        // Combine shards into a single buffer
        let mut buffer = Vec::with_capacity(TOTAL_SIZE);
        for shard in &shards {
            buffer.extend_from_slice(shard);
        }

        // Extract ID (terminating at null or $ padding)
        let id_bytes = &buffer[0..ID_SIZE];

        let id = Uuid::from_slice(id_bytes).unwrap().to_string();

        // Extract numeric fields
        let modification_count =
            i64::from_le_bytes(buffer[ID_SIZE..ID_SIZE + 8].try_into().map_err(|_| {
                ShardError::DecodingError("Failed to decode modification_count".to_string())
            })?);

        let seismic_activity =
            f64::from_le_bytes(buffer[ID_SIZE + 8..ID_SIZE + 16].try_into().map_err(|_| {
                ShardError::DecodingError("Failed to decode seismic_activity".to_string())
            })?);

        let temperature_c =
            f64::from_le_bytes(buffer[ID_SIZE + 16..ID_SIZE + 24].try_into().map_err(|_| {
                ShardError::DecodingError("Failed to decode temperature_c".to_string())
            })?);

        let radiation_level =
            f64::from_le_bytes(buffer[ID_SIZE + 24..ID_SIZE + 32].try_into().map_err(|_| {
                ShardError::DecodingError("Failed to decode radiation_level".to_string())
            })?);

        // Create and return the reconstructed struct
        Ok(Self {
            id,
            modification_count,
            seismic_activity,
            temperature_c,
            radiation_level,
        })
    }

    // Helper method to display the stats
    pub fn display(&self) -> String {
        format!(
            "Stats: id={}, mods={}, seismic={}, temp={}, rad={}",
            self.id,
            self.modification_count,
            self.seismic_activity,
            self.temperature_c,
            self.radiation_level
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- HAPPY PATH TESTS -----

    #[test]
    fn test_new_valid_id() {
        let result = EnrichedLocationStats::new("bac32c52-bb64-476d-a36d-91069bbd8a5e".to_string(), 42, 3.14, 25.5, 0.001);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.id, "bac32c52-bb64-476d-a36d-91069bbd8a5e");
        assert_eq!(stats.modification_count, 42);
        assert_eq!(stats.seismic_activity, 3.14);
        assert_eq!(stats.temperature_c, 25.5);
        assert_eq!(stats.radiation_level, 0.001);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        // Create original stats
        let original =
            EnrichedLocationStats::new("b6517ac5-2bf3-4a97-864a-ab4561381d5e".to_string(), 100, 5.678, -10.5, 0.0123).unwrap();

        // Encode to shards
        let shards = original.to_shards().unwrap();

        // Verify all shards are the same size
        for shard in &shards {
            assert_eq!(shard.len(), SHARD_SIZE);
        }

        // Decode back from shards
        let decoded = EnrichedLocationStats::from_shards(shards).unwrap();

        // Verify all fields match
        assert_eq!(decoded.id, original.id);
        assert_eq!(decoded.modification_count, original.modification_count);
        assert_eq!(decoded.seismic_activity, original.seismic_activity);
        assert_eq!(decoded.temperature_c, original.temperature_c);
        assert_eq!(decoded.radiation_level, original.radiation_level);
    }

  

    #[test]
    fn test_extreme_values() {
        // Test with extreme numeric values
        let stats = EnrichedLocationStats::new(
            "bac32c52-bb64-476d-a36d-91069bbd8a5e".to_string(),
            i64::MAX,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::NEG_INFINITY,
        )
        .unwrap();

        let shards = stats.to_shards().unwrap();
        let decoded = EnrichedLocationStats::from_shards(shards).unwrap();

        assert_eq!(decoded.id, "bac32c52-bb64-476d-a36d-91069bbd8a5e");
        assert_eq!(decoded.modification_count, i64::MAX);
        assert_eq!(decoded.seismic_activity, f64::MAX);
        assert_eq!(decoded.temperature_c, f64::MIN_POSITIVE);
        assert_eq!(decoded.radiation_level, f64::NEG_INFINITY);
    }

    // ----- SAD PATH TESTS -----


    #[test]
    fn test_from_shards_invalid_shard_size() {
        // Create invalid shards with incorrect sizes
        let shards = [
            vec![1, 2, 3],    // Too small
            vec![4, 5, 6],    // Too small
            vec![7, 8, 9],    // Too small
            vec![10, 11, 12], // Too small
        ];

        let result = EnrichedLocationStats::from_shards(shards);

        assert!(result.is_err());
        match result {
            Err(ShardError::InvalidShard(msg)) => {
                assert!(msg.contains("incorrect size"));
            }
            _ => panic!("Expected InvalidShard"),
        }
    }

    #[test]
    fn test_from_shards_inconsistent_sizes() {
        // Create a set of correctly-sized shards
        let valid_stats =
            EnrichedLocationStats::new("a6bdf0c3-8fbc-4371-8acd-c851e83fe248".to_string(), 42, 3.14, 25.5, 0.001).unwrap();

        let mut valid_shards = valid_stats.to_shards().unwrap();

        // Make one shard larger than the others
        valid_shards[2].push(99);

        let result = EnrichedLocationStats::from_shards(valid_shards);

        assert!(result.is_err());
        match result {
            Err(ShardError::InvalidShard(msg)) => {
                assert!(msg.contains("incorrect size"));
            }
            _ => panic!("Expected InvalidShard"),
        }
    }

    #[test]
    fn test_from_shards_corrupted_data() {
        // Create valid stats and encode to shards
        let stats =
            EnrichedLocationStats::new("a6bdf0c3-8fbc-4371-8acd-c851e83fe248".to_string(), 42, 3.14, 25.5, 0.001).unwrap();

        let mut shards = stats.to_shards().unwrap();

        // Corrupt the data in one of the shards that holds numeric data
        // (but keep the shard size the same)
        shards[2][0] = 0xFF;
        shards[2][1] = 0xFF;

        // Decoding should still work since we're not corrupting the structure
        let decoded = EnrichedLocationStats::from_shards(shards).unwrap();

        // But the values will be different from the original
        assert_eq!(decoded.id, "a6bdf0c3-8fbc-4371-8acd-c851e83fe248"); // ID should be the same
        assert!(
            decoded.seismic_activity != 3.14
                || decoded.temperature_c != 25.5
                || decoded.radiation_level != 0.001
        );
    }
}
