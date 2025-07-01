pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}

use crate::proto as root_proto;
use prost::Message;

pub fn decode_proto_message_from_bytes(
    bytes: &[u8],
) -> Result<root_proto::ProtoMessage, prost::DecodeError> {

    println!("before decode {:?} : {:?}", bytes.len(), &bytes);
    root_proto::ProtoMessage::decode(bytes).map_err(|e| {
        eprintln!("Decode error: {:?}", e);
        e
    })
}

pub fn decode_proto_message_from_vec(
    vec: Vec<u8>,
) -> Result<root_proto::ProtoMessage, prost::DecodeError> {
    root_proto::ProtoMessage::decode(vec.as_slice())
}

pub fn encode_proto_message(msg: &root_proto::ProtoMessage) -> Result<Vec<u8>, prost::EncodeError> {
    let mut buf: Vec<u8> = Vec::with_capacity(msg.encoded_len());
    msg.encode(&mut buf)?;
    println!("after encode {:?} : {:?}", buf.len(), &buf);
    Ok(buf)
}

pub fn create_text_proto_message(text: String) -> root_proto::ProtoMessage {
    root_proto::ProtoMessage {
        uuid: "some-unique-uuid".to_string(),
        sender_uuid: "sender-uuid".to_string(),
        timestamp: 0, // set your timestamp
        room_uuid: "room-uuid".to_string(),

        // Now explicitly refer to the root oneof enum:
        content: Some(root_proto::proto_message::Content::Text(
            root_proto::TextMessage { text },
        )),
    }
}


pub fn create_ack_proto_message(
    message_uuid: String,
    received: bool,
    read: bool,
) -> root_proto::ProtoMessage {
    root_proto::ProtoMessage {
        uuid: "some-unique-uuid".to_string(),
        sender_uuid: "sender-uuid".to_string(),
        timestamp: 0, // set your timestamp
        room_uuid: "room-uuid".to_string(),

        // Refer to the oneof enum variant for AckMessage:
        content: Some(root_proto::proto_message::Content::Status(
            root_proto::AckMessage {
                message_uuid,
                received,
                read,
            },
        )),
    }
}