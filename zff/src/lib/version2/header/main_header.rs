// - STD
use std::io::{Cursor, Read};

// - internal
use crate::{
	Result,
	ValueEncoder,
	ValueDecoder,
	HeaderCoding,
	ZffError,
	ZffErrorKind,
	Encryption,
	DEFAULT_LENGTH_HEADER_IDENTIFIER,
	DEFAULT_LENGTH_VALUE_HEADER_LENGTH,
	HEADER_IDENTIFIER_MAIN_HEADER,
	HEADER_IDENTIFIER_ENCRYPTED_MAIN_HEADER,
	ENCODING_KEY_DESCRIPTION_NOTES,
	ERROR_HEADER_DECODER_MAIN_HEADER_ENCRYPTED,
	ERROR_HEADER_DECODER_MISMATCH_IDENTIFIER,
	ERROR_HEADER_DECODER_MAIN_HEADER_NOT_ENCRYPTED,
};

use crate::version2::header::{
	CompressionHeader,
	EncryptionHeader,
};

// - external
use serde::ser::{Serialize, Serializer, SerializeStruct};
use byteorder::{ReadBytesExt, BigEndian};

/// The main header is the first Header, which can be found at the beginning of the first segment.\
/// This header contains a lot of other headers (e.g. compression header, ...) and start information.
#[derive(Debug,Clone)]
pub struct MainHeader {
	version: u8,
	encryption_header: Option<EncryptionHeader>,
	compression_header: CompressionHeader,
	chunk_size: u8,
	signature_flag: u8,
	segment_size: u64,
	unique_identifier: i64,
	description_notes: Option<String>,
}

impl MainHeader {
	/// returns a new main header with the given values.
	pub fn new(
		version: u8,
		encryption_header: Option<EncryptionHeader>,
		compression_header: CompressionHeader,
		chunk_size: u8,
		signature_flag: u8,
		segment_size: u64,
		unique_identifier: i64,
		description_notes: Option<String>) -> MainHeader {
		Self {
			version: version,
			encryption_header: encryption_header,
			compression_header: compression_header,
			chunk_size: chunk_size,
			signature_flag: signature_flag,
			segment_size: segment_size,
			unique_identifier: unique_identifier,
			description_notes: description_notes,
		}
	}

	/// returns the header identifier for the encrypted main header.
	pub fn encrypted_header_identifier() -> u32 {
		HEADER_IDENTIFIER_ENCRYPTED_MAIN_HEADER
	}

	fn encode_encrypted_header<K>(&self, key: K) -> Result<Vec<u8>>
	where
		K: AsRef<[u8]>
	{
		let mut vec = Vec::new();
		vec.push(self.version);
		let encryption_header = match &self.encryption_header {
			None => return Err(ZffError::new(ZffErrorKind::MissingEncryptionHeader, "")),
			Some(header) => {
				header
			}
		};
		let encryption_flag: u8 = 2;
		vec.push(encryption_flag);
		vec.append(&mut encryption_header.encode_directly());

		let mut data_to_encrypt = Vec::new();
		data_to_encrypt.append(&mut self.encode_content());

		let encrypted_data = Encryption::encrypt_header(
			key, data_to_encrypt,
			encryption_header.nonce(),
			encryption_header.algorithm()
			)?;
		vec.append(&mut encrypted_data.encode_directly());
		return Ok(vec);
	}

	fn check_encrypted_identifier<R: Read>(data: &mut R) -> bool {
		let identifier = match data.read_u32::<BigEndian>() {
			Ok(val) => val,
			Err(_) => return false,
		};
		if identifier == Self::encrypted_header_identifier() { 
			return true;
		} else {
			return false;
		}
	}

	/// decodes the encrypted main header with the given password.
	pub fn decode_encrypted_header_with_password<R, P>(data: &mut R, password: P) -> Result<MainHeader>
	where
		R: Read,
		P: AsRef<[u8]>,
	{
		if !Self::check_encrypted_identifier(data) {
			return Err(ZffError::new(ZffErrorKind::HeaderDecodeMismatchIdentifier, ERROR_HEADER_DECODER_MISMATCH_IDENTIFIER));
		};
		let header_length = Self::decode_header_length(data)? as usize;
		let mut header_content = vec![0u8; header_length-DEFAULT_LENGTH_HEADER_IDENTIFIER-DEFAULT_LENGTH_VALUE_HEADER_LENGTH];
		data.read_exact(&mut header_content)?;
		let mut cursor = Cursor::new(header_content);
		let header_version = u8::decode_directly(&mut cursor)?;
		let encryption_flag = u8::decode_directly(&mut cursor)?;
		if encryption_flag != 2 {
			return Err(ZffError::new(ZffErrorKind::HeaderDecodeEncryptedMainHeader, ERROR_HEADER_DECODER_MAIN_HEADER_NOT_ENCRYPTED));
		}
		let encryption_header = EncryptionHeader::decode_directly(&mut cursor)?;
		let encrypted_data = Vec::<u8>::decode_directly(&mut cursor)?;
		let encryption_key = encryption_header.decrypt_encryption_key(password)?;
		let nonce = encryption_header.nonce();
		let algorithm = encryption_header.algorithm();
		let decrypted_data = Encryption::decrypt_header(encryption_key, encrypted_data, nonce, algorithm)?;
		let mut cursor = Cursor::new(decrypted_data);
		let (compression_header,
			chunk_size,
			signature_flag,
			segment_size,
			unique_identifier,
			description_notes) = Self::decode_inner_content(&mut cursor)?;
		let main_header = Self::new(
			header_version,
			Some(encryption_header),
			compression_header,
			chunk_size,
			signature_flag,
			segment_size,
			unique_identifier,
			description_notes);
		Ok(main_header)
	}

	/// encodes the main header to a ```Vec<u8>```. The encryption flag will be set to 2.
	/// # Error
	/// The method returns an error, if the encryption header is missing (=None).
	pub fn encode_encrypted_header_directly<K>(&self, key: K) -> Result<Vec<u8>>
	where
		K: AsRef<[u8]>,
	{
		let mut vec = Vec::new();
		let mut encoded_header = self.encode_encrypted_header(key)?;
		let identifier = HEADER_IDENTIFIER_ENCRYPTED_MAIN_HEADER;
		let encoded_header_length = 4 + 8 + (encoded_header.len() as u64); //4 bytes identifier + 8 bytes for length + length itself
		vec.append(&mut identifier.to_be_bytes().to_vec());
		vec.append(&mut encoded_header_length.to_le_bytes().to_vec());
		vec.append(&mut encoded_header);

		Ok(vec)
	}

	fn encode_content(&self) -> Vec<u8> {
		let mut vec = Vec::new();
		
		vec.append(&mut self.compression_header.encode_directly());
		vec.push(self.chunk_size);
		vec.push(self.signature_flag);
		vec.append(&mut self.segment_size.encode_directly());
		vec.append(&mut self.unique_identifier.encode_directly());
		if let Some(description_notes) = &self.description_notes {
			vec.append(&mut description_notes.encode_for_key(ENCODING_KEY_DESCRIPTION_NOTES));
		};
		vec
	}

	fn decode_inner_content<R: Read>(inner_content: &mut R) -> Result<(
		CompressionHeader,
		u8, // chunk size
		u8, // signature flag
		u64, // segment size
		i64, // unique identifier
		Option<String>, //Description notes
		)>{
		let compression_header = CompressionHeader::decode_directly(inner_content)?;
		let chunk_size = u8::decode_directly(inner_content)?;
		let signature_flag = u8::decode_directly(inner_content)?;
		let segment_size = u64::decode_directly(inner_content)?;
		let unique_identifier = i64::decode_directly(inner_content)?;
		let description_notes = match String::decode_for_key(inner_content, ENCODING_KEY_DESCRIPTION_NOTES) {
			Ok(value) => Some(value),
			Err(e) => match e.get_kind() {
				ZffErrorKind::HeaderDecoderKeyNotOnPosition => None,
				_ => return Err(e)
			},
		};
		let inner_content = (
			compression_header,
			chunk_size,
			signature_flag,
			segment_size,
			unique_identifier,
			description_notes);
		Ok(inner_content)
	}

	/// returns the chunk_size.
	pub fn chunk_size(&self) -> usize {
		1<<self.chunk_size
	}

	/// returns the segment size
	pub fn segment_size(&self) -> u64 {
		self.segment_size.clone()
	}

	/// returns the len() of the ```Vec<u8>``` (encoded main header).
	pub fn get_encoded_size(&self) -> usize {
		self.encode_directly().len()
	}

	/// returns the len() of the ```Vec<u8>``` (encoded encrypted main header).
	/// # Error
	/// The method fails, if the encryption fails or no encryption header is present.
	pub fn get_encrypted_encoded_size<K>(&self, key: K) -> Result<usize>
	where
		K: AsRef<[u8]>,
	{
		Ok(self.encode_encrypted_header_directly(key)?.len())
	}

	/// returns, if the chunks has a ed25519 signature or not.
	pub fn has_signature(&self) -> bool {
		self.signature_flag != 0
	}

	/// returns a reference to the inner compression header
	pub fn compression_header(&self) -> &CompressionHeader {
		&self.compression_header
	}

	/// returns a reference to the inner encryption header (if available)
	pub fn encryption_header(&self) -> &Option<EncryptionHeader> {
		&self.encryption_header
	}

	/// returns the unique identifier
	pub fn unique_identifier(&self) -> i64 {
		self.unique_identifier
	}
}

impl HeaderCoding for MainHeader {
	type Item = MainHeader;

	fn identifier() -> u32 {
		HEADER_IDENTIFIER_MAIN_HEADER
	}

	fn version(&self) -> u8 {
		self.version
	}

	fn encode_header(&self) -> Vec<u8> {
		let mut vec = Vec::new();

		vec.push(self.version);
		match &self.encryption_header {
			None => {
				let encryption_flag: u8 = 0;
				vec.push(encryption_flag);
			},
			Some(header) => {
				let encryption_flag: u8 = 1;
				vec.push(encryption_flag);
				vec.append(&mut header.encode_directly());
			},
		};

		vec.append(&mut self.encode_content());

		vec
	}

	fn decode_content(data: Vec<u8>) -> Result<MainHeader> {
		let mut cursor = Cursor::new(data);
		let version = u8::decode_directly(&mut cursor)?;
		//encryption flag:
		let mut encryption_header = None;
		let encryption_flag = u8::decode_directly(&mut cursor)?;
		if encryption_flag == 1 {
			encryption_header = Some(EncryptionHeader::decode_directly(&mut cursor)?);
		} else if encryption_flag > 1 {
			return Err(ZffError::new(ZffErrorKind::HeaderDecodeEncryptedMainHeader, ERROR_HEADER_DECODER_MAIN_HEADER_ENCRYPTED))
		}
		let (compression_header,
			chunk_size,
			signature_flag,
			segment_size,
			unique_identifier,
			description_notes) = Self::decode_inner_content(&mut cursor)?;
		let main_header = Self::new(
			version,
			encryption_header,
			compression_header,
			chunk_size,
			signature_flag,
			segment_size,
			unique_identifier,
			description_notes);
		Ok(main_header)
	}
}

impl Serialize for MainHeader {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("MainHeader", 10)?;
        state.serialize_field("header_version", &self.version)?;
        state.serialize_field("encryption", &self.encryption_header)?;
        state.serialize_field("compression", &self.compression_header)?;

        state.serialize_field("chunk_size", &(1<<&self.chunk_size))?;

        state.serialize_field("signature_flag", &(self.signature_flag != 0))?;
        state.serialize_field("segment_size", &self.segment_size.to_string())?;
        state.serialize_field("unique_identifier", &self.unique_identifier)?;

        state.end()
    }
}