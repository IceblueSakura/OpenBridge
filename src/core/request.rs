use bytes::Bytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    ChatCompletions,
    Responses,
}

#[derive(Clone, Debug)]
pub struct ValidatedRequest {
    protocol: Protocol,
    body: Bytes,
}

impl ValidatedRequest {
    pub fn new(protocol: Protocol, body: Bytes) -> Self {
        Self { protocol, body }
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
