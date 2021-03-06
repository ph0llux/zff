// - STD
use std::io::{Cursor,Read};

// - external
use ed25519_dalek::{SIGNATURE_LENGTH};

// - internal
use crate::{
	Result,
	HeaderCoding,
	ValueEncoder,
	ValueDecoder,
	HEADER_IDENTIFIER_CHUNK_HEADER,
	ERROR_FLAG_VALUE,
	COMPRESSION_FLAG_VALUE
};

/// Header for chunk data.\
/// Each data chunk has his own chunk header. After the header, the chunked data follows.
#[derive(Debug,Clone)]
pub struct ChunkHeader {
	version: u8,
	chunk_number: u64,
	chunk_size: u64,
	crc32: u32,
	error_flag: bool,
	compression_flag: bool,
	ed25519_signature: Option<[u8; SIGNATURE_LENGTH]>,
}

impl ChunkHeader {
	/// creates a new empty chunk header with a given chunk number. All other values are set to ```0``` or ```None```.
	pub fn new_empty(version: u8, chunk_number: u64) -> ChunkHeader {
		Self {
			version,
			chunk_number,
			chunk_size: 0,
			crc32: 0,
			error_flag: false,
			compression_flag: false,
			ed25519_signature: None,
		}
	}

	/// creates a new header from the given data.
	pub fn new(version: u8, chunk_number: u64, chunk_size: u64, crc32: u32, error_flag: bool, compression_flag: bool, ed25519_signature: Option<[u8; SIGNATURE_LENGTH]>) -> ChunkHeader {
		Self {
			version,
			chunk_number,
			chunk_size,
			crc32,
			error_flag,
			compression_flag,
			ed25519_signature
		}
	}

	/// overwrites the chunk size in the header with the given value. This can be useful, if you create an 'empty'
	/// header (with size=0) and want to set the size after reading the data from source to buffer.
	pub fn set_chunk_size(&mut self, size: u64) {
		self.chunk_size = size
	}

	/// returns chunk size, excluding the header size.
	pub fn chunk_size(&self) -> &u64 {
		&self.chunk_size
	}

	/// overwrites the crc32 value in the header with the given value. This can be useful, if you create an 'empty'
	/// header (with crc32=0) and want to set the crc32 value after reading the data from source to buffer.
	pub fn set_crc32(&mut self, crc32: u32) {
		self.crc32 = crc32
	}

	/// returns the crc32 value.
	pub fn crc32(&self) -> u32 {
		self.crc32
	}

	/// returns if an error flag is set for this chunk.
	pub fn error_flag(&self) -> bool {
		self.error_flag
	}

	/// sets the compression_flag
	pub fn set_compression_flag(&mut self) {
		self.compression_flag = true;
	}

	/// returns the compression flag value
	pub fn compression_flag(&self) -> bool {
		self.compression_flag
	}

	/// overwrites the signature in the header with the given value. This can be useful, if you create an 'empty'
	/// header (with signature=None) and want to set the signature after reading the data from source to buffer.
	/// Note: The Ed25519 signature per chunk is **optional**, so you have to set the signature as an ```Option<[u8; 64]>```.
	pub fn set_signature(&mut self, signature: Option<[u8; SIGNATURE_LENGTH]>) {
		self.ed25519_signature = signature
	}

	/// returns the chunk number of the chunk (header).
	pub fn chunk_number(&self) -> u64 {
		self.chunk_number
	}

	/// returns the signature, if available
	pub fn signature(&self) -> &Option<[u8; SIGNATURE_LENGTH]> {
		&self.ed25519_signature
	}
}

impl HeaderCoding for ChunkHeader {
	type Item = ChunkHeader;

	fn identifier() -> u32 {
		HEADER_IDENTIFIER_CHUNK_HEADER
	}

	fn version(&self) -> u8 {
		self.version
	}

	fn encode_header(&self) -> Vec<u8> {
		let mut vec = vec![self.version];

		vec.append(&mut self.chunk_number.encode_directly());
		vec.append(&mut self.chunk_size.encode_directly());
		vec.append(&mut self.crc32.encode_directly());
		let mut flags: u8 = 0;
		if self.error_flag {
			flags += ERROR_FLAG_VALUE;
		};
		if self.compression_flag {
			flags += COMPRESSION_FLAG_VALUE;
		};
		vec.append(&mut flags.encode_directly());
		match self.ed25519_signature {
			None => (),
			Some(signature) => vec.append(&mut signature.encode_directly()),
		};
		
		vec
	}

	fn decode_content(data: Vec<u8>) -> Result<ChunkHeader> {
		let mut cursor = Cursor::new(&data);
		let version = u8::decode_directly(&mut cursor)?;
		let chunk_number = u64::decode_directly(&mut cursor)?;
		let chunk_size = u64::decode_directly(&mut cursor)?;
		let crc32 = u32::decode_directly(&mut cursor)?;
		let flags = u8::decode_directly(&mut cursor)?;
		let compression_flag = flags & COMPRESSION_FLAG_VALUE != 0;
		let error_flag = flags & ERROR_FLAG_VALUE != 0;
		let mut ed25519_signature = None;
		if cursor.position() < (data.len() as u64 - 1) {
			let mut buffer = [0; SIGNATURE_LENGTH];
			cursor.read_exact(&mut buffer)?;
			ed25519_signature = Some(buffer);
		}

		Ok(ChunkHeader::new(version, chunk_number, chunk_size, crc32, error_flag, compression_flag, ed25519_signature))
	}
}