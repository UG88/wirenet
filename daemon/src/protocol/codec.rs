use super::messages::Message;
use anyhow::{Context, Result};
use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

#[derive(Default)]
pub struct WireNetCodec {
    length_codec: LengthDelimitedCodec,
}

impl WireNetCodec {
    pub fn new() -> Self {
        Self {
            length_codec: LengthDelimitedCodec::new(),
        }
    }
}

impl Decoder for WireNetCodec {
    type Item = Message;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        match self.length_codec.decode(src)? {
            Some(frame) => {
                let msg = serde_json::from_slice(&frame)
                    .context("Failed to deserialize WireNet protocol message")?;
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}

impl Encoder<Message> for WireNetCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<()> {
        let serialized =
            serde_json::to_vec(&item).context("Failed to serialize WireNet protocol message")?;
        self.length_codec.encode(serialized.into(), dst)?;
        Ok(())
    }
}
