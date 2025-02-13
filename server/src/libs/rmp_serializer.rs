pub struct RmpSerde;

impl<T: serde::Serialize> native_model::Encode<T> for RmpSerde {
    type Error = rmp_serde::encode::Error;
    fn encode(obj: &T) -> Result<Vec<u8>, Self::Error> {
        rmp_serde::encode::to_vec(obj)
    }
}

impl<T: for<'de> serde::Deserialize<'de>> native_model::Decode<T> for RmpSerde {
    type Error = rmp_serde::decode::Error;
    fn decode(data: Vec<u8>) -> Result<T, Self::Error> {
        rmp_serde::decode::from_slice(&data)
    }
}