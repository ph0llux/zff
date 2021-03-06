// - STD
use std::io::{Cursor, Read};
use std::collections::HashMap;
use std::fmt;

// - internal
use crate::{
	Result,
	HeaderCoding,
	ValueDecoder,
	ValueEncoder,
	Encryption,
	ZffError,
	ZffErrorKind,
};

use crate::{
	DEFAULT_LENGTH_HEADER_IDENTIFIER,
	DEFAULT_LENGTH_VALUE_HEADER_LENGTH,
	HEADER_IDENTIFIER_FILE_HEADER,
	ERROR_HEADER_DECODER_MISMATCH_IDENTIFIER
};

use crate::header::{
	EncryptionHeader,
};

/// Defines all file types, which are implemented for zff files.
#[repr(u8)]
#[non_exhaustive]
#[derive(Debug,Clone,Eq,PartialEq,Hash)]
pub enum FileType {
	/// Represents a regular file (e.g. like "textfile.txt").
	File = 1,
	/// Represents a directory.
	Directory = 2,
	/// Represents a symbolic link.
	Symlink = 3,
	/// Represents a hard link (mostly used at unix like operating systems).
	Hardlink = 4,
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let msg = match self {
			FileType::File => "File",
			FileType::Directory => "Directory",
			FileType::Symlink => "Symlink",
			FileType::Hardlink => "Hardlink",
		};
		write!(f, "{}", msg)
	}
}

/// Each dumped file* contains a [FileHeader] containing several metadata.
/// The following metadata are included in a [FileHeader]:
/// - the internal file number of the appropriate file.
/// - the [FileType] of the appropriate file.
/// - the original filename of the appropriate file **without** the full path (just the filename, e.g. "my_texfile.txt" or "my_directory")
/// - the file number of the parent directory of this file (if the file lies into the root directory, this is 0 because the first valid file number in zff is 1).
/// - the atime, mtime, ctime and btime.
/// - A HashMap to extend the metadata based on the operating system/filesystem. Some fields are predefined, see [the full list in the wiki](https://github.com/ph0llux/zff/wiki/zff-header-layout#file-metadata-extended-information)
#[derive(Debug,Clone,Eq,PartialEq)]
pub struct FileHeader {
	version: u8,
	file_number: u64,
	file_type: FileType,
	filename: String,
	parent_file_number: u64,
	atime: u64,
	mtime: u64,
	ctime: u64,
	btime: u64,
	metadata_ext: HashMap<String, String>,
}

impl FileHeader {
	/// creates a new [FileHeader] with the given values.
	pub fn new<F: Into<String>>(
		version: u8,
		file_number: u64,
		file_type: FileType,
		filename: F,
		parent_file_number: u64,
		atime: u64,
		mtime: u64,
		ctime: u64,
		btime: u64,
		metadata_ext: HashMap<String, String>) -> FileHeader {
		Self {
			version,
			file_number,
			file_type,
			filename: filename.into(),
			parent_file_number,
			atime,
			mtime,
			ctime,
			btime,
			metadata_ext
		}
	}
	/// returns the file number
	pub fn file_number(&self) -> u64 {
		self.file_number
	}
	/// returns the [FileType]
	pub fn file_type(&self) -> FileType {
		self.file_type.clone()
	}
	/// returns the filename
	pub fn filename(&self) -> &str {
		&self.filename
	}
	/// returns the file number of the parent directory
	pub fn parent_file_number(&self) -> u64 {
		self.parent_file_number
	}
	/// returns the atime
	pub fn atime(&self) -> u64 {
		self.atime
	}
	/// returns the mtime 
	pub fn mtime(&self) -> u64 {
		self.mtime
	}
	/// returns the ctime
	pub fn ctime(&self) -> u64 {
		self.ctime
	}
	/// returns the btime
	pub fn btime(&self) -> u64 {
		self.btime
	}
	/// returns the extended metadata [HashMap] as a reference.
	pub fn metadata_ext(&self) -> &HashMap<String, String> {
		&self.metadata_ext
	}

	/// transforms the inner [FileType] to a [FileType::Hardlink]. This does not work with a [FileType::Symlink]!
	pub fn transform_to_hardlink(&mut self) {
		if self.file_type != FileType::Symlink {
			self.file_type = FileType::Hardlink
		}
	}

	/// encodes the file header to a ```Vec<u8>```. The encryption flag of the appropriate object header has to be set to 2.
	/// # Error
	/// The method returns an error, if the encryption fails.
	pub fn encode_encrypted_header_directly<K>(&self, key: K, encryption_header: EncryptionHeader) -> Result<Vec<u8>>
	where
		K: AsRef<[u8]>,
	{
		let mut vec = Vec::new();
		let mut encoded_header = self.encode_encrypted_header(key, encryption_header)?;
		let identifier = HEADER_IDENTIFIER_FILE_HEADER;
		let encoded_header_length = 4 + 8 + (encoded_header.len() as u64); //4 bytes identifier + 8 bytes for length + length itself
		vec.append(&mut identifier.to_be_bytes().to_vec());
		vec.append(&mut encoded_header_length.to_le_bytes().to_vec());
		vec.append(&mut encoded_header);

		Ok(vec)
	}

	fn encode_encrypted_header<K>(&self, key: K, encryption_header: EncryptionHeader) -> Result<Vec<u8>>
	where
		K: AsRef<[u8]>
	{
		let mut vec = Vec::new();
		vec.append(&mut self.version.encode_directly());
		vec.append(&mut self.file_number.encode_directly());

		let mut data_to_encrypt = Vec::new();
		data_to_encrypt.append(&mut self.encode_content());

		let encrypted_data = Encryption::encrypt_header(
			key, data_to_encrypt,
			encryption_header.nonce(),
			encryption_header.algorithm()
			)?;
		vec.append(&mut encrypted_data.encode_directly());
		Ok(vec)
	}

	fn encode_content(&self) -> Vec<u8> {
		let mut vec = Vec::new();
		vec.append(&mut (self.file_type.clone() as u8).encode_directly());
		vec.append(&mut self.filename().encode_directly());
		vec.append(&mut self.parent_file_number.encode_directly());
		vec.append(&mut self.atime.encode_directly());
		vec.append(&mut self.mtime.encode_directly());
		vec.append(&mut self.ctime.encode_directly());
		vec.append(&mut self.btime.encode_directly());
		vec.append(&mut self.metadata_ext.encode_directly());
		vec
	}

	/// decodes the encrypted header with the given key and [crate::header::EncryptionHeader].
	/// The appropriate [crate::header::EncryptionHeader] has to be stored in the appropriate [crate::header::ObjectHeader].
	pub fn decode_encrypted_header_with_key<R, K>(data: &mut R, key: K, encryption_header: EncryptionHeader) -> Result<FileHeader>
	where
		R: Read,
		K: AsRef<[u8]>,
	{
		if !Self::check_identifier(data) {
			return Err(ZffError::new(ZffErrorKind::HeaderDecodeMismatchIdentifier, ERROR_HEADER_DECODER_MISMATCH_IDENTIFIER));
		};
		let header_length = Self::decode_header_length(data)? as usize;
		let mut header_content = vec![0u8; header_length-DEFAULT_LENGTH_HEADER_IDENTIFIER-DEFAULT_LENGTH_VALUE_HEADER_LENGTH];
		data.read_exact(&mut header_content)?;
		let mut cursor = Cursor::new(header_content);
		let header_version = u8::decode_directly(&mut cursor)?;
		let file_number = u64::decode_directly(&mut cursor)?;
		
		let encrypted_data = Vec::<u8>::decode_directly(&mut cursor)?;
		let nonce = encryption_header.nonce();
		let algorithm = encryption_header.algorithm();
		let decrypted_data = Encryption::decrypt_header(key, encrypted_data, nonce, algorithm)?;
		let mut cursor = Cursor::new(decrypted_data);
		let (file_type,
			filename,
			parent_file_number,
			atime,
			mtime,
			ctime,
			btime,
			metadata_ext) = Self::decode_inner_content(&mut cursor)?;
		let file_header = Self::new(
			header_version,
			file_number,
			file_type,
			filename,
			parent_file_number,
			atime,
			mtime,
			ctime,
			btime,
			metadata_ext);
		Ok(file_header)
	}

	fn decode_inner_content<R: Read>(inner_content: &mut R) -> Result<(
		FileType,
		String, //Filename
		u64, //parent_file_number
		u64, //atime,
		u64, //mtime
		u64, //ctime,
		u64, //btime,
		HashMap<String, String>,
		)> {
		let file_type = match u8::decode_directly(inner_content)? {
			1 => FileType::File,
			2 => FileType::Directory,
			3 => FileType::Symlink,
			4 => FileType::Hardlink,
			val => return Err(ZffError::new(ZffErrorKind::UnknownFileType, val.to_string()))
		};
		let filename = String::decode_directly(inner_content)?;
		let parent_file_number = u64::decode_directly(inner_content)?;
		let atime = u64::decode_directly(inner_content)?;
		let mtime = u64::decode_directly(inner_content)?;
		let ctime = u64::decode_directly(inner_content)?;
		let btime = u64::decode_directly(inner_content)?;
		let metadata_ext = HashMap::<String, String>::decode_directly(inner_content)?;

		let inner_content = (
			file_type,
			filename,
			parent_file_number,
			atime,
			mtime,
			ctime,
			btime,
			metadata_ext);
		Ok(inner_content)
	}
}

impl HeaderCoding for FileHeader {
	type Item = FileHeader;
	
	fn identifier() -> u32 {
		HEADER_IDENTIFIER_FILE_HEADER
	}

	fn version(&self) -> u8 {
		self.version
	}
	
	fn encode_header(&self) -> Vec<u8> {
		let mut vec = Vec::new();
		vec.append(&mut self.version.encode_directly());
		vec.append(&mut self.file_number.encode_directly());
		vec.append(&mut self.encode_content());
		vec
		
	}

	fn decode_content(data: Vec<u8>) -> Result<FileHeader> {
		let mut cursor = Cursor::new(data);
		let header_version = u8::decode_directly(&mut cursor)?;
		let file_number = u64::decode_directly(&mut cursor)?;
		let (file_type, filename, parent_file_number, atime, mtime, ctime, btime, metadata_ext) = Self::decode_inner_content(&mut cursor)?;
		Ok(FileHeader::new(header_version, file_number, file_type, filename, parent_file_number, atime, mtime, ctime, btime, metadata_ext))
	}
}