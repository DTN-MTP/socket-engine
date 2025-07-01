pub mod encoding;
pub mod endpoint;
pub mod engine;
pub mod socket;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}
