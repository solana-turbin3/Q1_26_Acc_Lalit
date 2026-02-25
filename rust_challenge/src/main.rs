use serde::{Serialize as SerdeSerialize, de::DeserializeOwned as SerdeDeserializeOwned};
use borsh::{BorshSerialize, BorshDeserialize};
use wincode::{Serialize as WincodeSerialize, DeserializeOwned as WincodeDeserialize}; 

fn main() {
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
    };

    // Borsh
    let mut borsh_storage = Storage::new(BorshSerializer);
    borsh_storage.save(&person).unwrap();
    let loaded_person_borsh = borsh_storage.load().unwrap();
    println!("Borsh loaded person: {:?}", loaded_person_borsh);

    // JSON
    let mut json_storage = Storage::new(JsonSerializer);
    json_storage.save(&person).unwrap();
    let loaded_person_json = json_storage.load().unwrap();
    println!("JSON loaded person: {:?}", loaded_person_json);

    // Wincode
    let mut wincode_storage = Storage::new(WincodeSerializer);
    wincode_storage.save(&person).unwrap();
    let loaded_person_wincode = wincode_storage.load().unwrap();
    println!("Wincode loaded person: {:?}", loaded_person_wincode);
}


pub trait Serializer<T> {
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn from_bytes(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error>>;
}

// implemnt serializer for borsh, json and wincode
struct BorshSerializer;
struct JsonSerializer;
struct WincodeSerializer;

impl<T: BorshSerialize + BorshDeserialize> Serializer<T> for BorshSerializer {
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(borsh::to_vec(value)?)
    }
    fn from_bytes(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error>> {
        Ok(borsh::from_slice(bytes)?)
    }
}

impl<T: SerdeSerialize + SerdeDeserializeOwned> Serializer<T> for JsonSerializer {
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(serde_json::to_vec(value)?)
    }
    fn from_bytes(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

impl<T: WincodeSerialize<Src = T> + WincodeDeserialize<Dst = T>> Serializer<T> for WincodeSerializer {
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(wincode::serialize(value)?)
    }
    fn from_bytes(&self, bytes: &[u8]) -> Result<T, Box<dyn std::error::Error>> {
        Ok(wincode::deserialize(bytes)?)
    }
}

struct Storage<T, S> {
    data: Option<Vec<u8>>,
    serializer: S,
    _marker: std::marker::PhantomData<T>,
}

impl<T, S: Serializer<T>> Storage<T, S> {
    fn new(serializer: S) -> Self {
        Self {
            data: None,
            serializer,
            _marker: std::marker::PhantomData,
        }
    }
    fn save(&mut self, value: &T) -> Result<(), Box<dyn std::error::Error>> {
        self.data = Some(self.serializer.to_bytes(value)?);
        Ok(())
    }

    fn load(&self) -> Result<T, Box<dyn std::error::Error>> {
        if let Some(ref bytes) = self.data {
            self.serializer.from_bytes(bytes)
        } else {
            Err("No data to load".into())
        }
    }

    fn has_data(&self) -> bool {
        self.data.is_some()
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize, wincode::SchemaWrite, wincode::SchemaRead)]
struct Person {
    name: String,
    age: u32,
}