#[cfg(test)]
mod handshake_message_client_hello_test;

use super::handshake_random::*;
use super::*;
use crate::cipher_suite::*;
use crate::compression_methods::*;
use crate::extension::*;
use crate::record_layer::record_layer_header::*;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::fmt;
use std::io::{BufReader, BufWriter};

/*
When a client first connects to a server it is required to send
the client hello as its first message.  The client can also send a
client hello in response to a hello request or on its own
initiative in order to renegotiate the security parameters in an
existing connection.
*/
//#[derive(Debug)]
pub struct HandshakeMessageClientHello {
    version: ProtocolVersion,
    random: HandshakeRandom,
    cookie: Vec<u8>,

    cipher_suites: Vec<Box<dyn CipherSuite>>,
    compression_methods: CompressionMethods,
    extensions: Vec<Extension>,
}

impl PartialEq for HandshakeMessageClientHello {
    fn eq(&self, other: &Self) -> bool {
        let is_eq = self.version == other.version
            && self.random == other.random
            && self.cookie == other.cookie
            && self.compression_methods == other.compression_methods
            && self.extensions == other.extensions;

        is_eq
    }
}

impl fmt::Debug for HandshakeMessageClientHello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cipher_suites_str = String::new();
        for cipher_suite in &self.cipher_suites {
            cipher_suites_str += &cipher_suite.to_string();
            cipher_suites_str += " ";
        }
        let s = vec![
            format!("version: {:?} random: {:?}", self.version, self.random),
            format!("cookie: {:?}", self.cookie),
            format!("cipher_suites: {:?}", cipher_suites_str),
            format!("compression_methods: {:?}", self.compression_methods),
            format!("extensions: {:?}", self.extensions),
        ];
        write!(f, "{}", s.join(" "))
    }
}

const HANDSHAKE_MESSAGE_CLIENT_HELLO_VARIABLE_WIDTH_START: usize = 34;

impl HandshakeMessageClientHello {
    fn handshake_type() -> HandshakeType {
        HandshakeType::ClientHello
    }

    pub fn marshal<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        if self.cookie.len() > 255 {
            return Err(ERR_COOKIE_TOO_LONG.clone());
        }

        writer.write_u8(self.version.major)?;
        writer.write_u8(self.version.minor)?;
        self.random.marshal(writer)?;

        // SessionID
        writer.write_u8(0x00)?;

        writer.write_u8(self.cookie.len() as u8)?;
        writer.write_all(&self.cookie)?;

        writer.write_u16::<BigEndian>(self.cipher_suites.len() as u16)?;
        for cipher_suite in &self.cipher_suites {
            writer.write_u16::<BigEndian>(cipher_suite.id() as u16)?;
        }

        self.compression_methods.marshal(writer)?;

        let mut extension_buffer = vec![];
        {
            let mut extension_writer = BufWriter::<&mut Vec<u8>>::new(extension_buffer.as_mut());
            for extension in &self.extensions {
                extension.marshal(&mut extension_writer)?;
            }
        }

        writer.write_u16::<BigEndian>(extension_buffer.len() as u16)?;
        writer.write_all(&extension_buffer)?;

        Ok(())
    }

    pub fn unmarshal<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let major = reader.read_u8()?;
        let minor = reader.read_u8()?;
        let random = HandshakeRandom::unmarshal(reader)?;

        // Session ID
        reader.read_u8()?;

        let cookie_len = reader.read_u8()? as usize;
        let mut cookie = vec![0; cookie_len];
        reader.read_exact(&mut cookie)?;

        let cipher_suites_len = reader.read_u16::<BigEndian>()? as usize / 2;
        let mut cipher_suites = vec![];
        for _ in 0..cipher_suites_len {
            let id: CipherSuiteID = reader.read_u16::<BigEndian>()?.into();
            let cipher_suite = cipher_suite_for_id(id)?;
            cipher_suites.push(cipher_suite);
        }

        let compression_methods = CompressionMethods::unmarshal(reader)?;
        let mut extensions = vec![];

        let extension_buffer_len = reader.read_u16::<BigEndian>()? as usize;
        let mut extension_buffer = vec![0u8; extension_buffer_len];
        reader.read_exact(&mut extension_buffer)?;

        let mut extension_reader = BufReader::new(extension_buffer.as_slice());
        let mut offset = 0;
        while offset < extension_buffer_len {
            let extension = Extension::unmarshal(&mut extension_reader)?;
            extensions.push(extension);

            let extension_len =
                u16::from_be_bytes([extension_buffer[offset + 2], extension_buffer[offset + 3]])
                    as usize;
            offset += 4 + extension_len;
        }

        Ok(HandshakeMessageClientHello {
            version: ProtocolVersion { major, minor },
            random,
            cookie,

            cipher_suites,
            compression_methods,
            extensions,
        })
    }
}