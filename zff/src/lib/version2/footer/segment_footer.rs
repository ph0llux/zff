// - STD
use std::io::{Cursor};

// - internal
use crate::version1::{
	Result,
	HeaderCoding,
	ValueEncoder,
	ValueDecoder,

};

use crate::{
	FOOTER_IDENTIFIER_SEGMENT_FOOTER,
};

/// The SegmentFooter is a footer which is be written at the end of each segment.
/// The footer contains a table on the chunks, present in the appropriate segment.
/// The offset table is internally managed as a ```Vec<u64>```.
pub struct SegmentFooter {
	version: u8,
	length_of_segment: u64,
	chunk_offsets: Vec<u64>,
	/// The offset where the footer starts.
	footer_offset: u64,

}

impl SegmentFooter {
	/// creates a new empty SegmentFooter.
	pub fn new_empty(version: u8) -> SegmentFooter {
		Self {
			version: version,
			length_of_segment: 0,
			chunk_offsets: Vec::new(),
			footer_offset: 0,
		}
	}

	/// creates a new SegmentFooter with a given "offset table" (represented as ```Vec<u64>```.
	pub fn new(version: u8, length_of_segment: u64, chunk_offsets: Vec<u64>, footer_offset: u64) -> SegmentFooter {
		Self {
			version: version,
			length_of_segment: length_of_segment,
			chunk_offsets: chunk_offsets,
			footer_offset: footer_offset,
		}
	}

	/// returns the length of the segment in bytes.
	pub fn length_of_segment(&self) -> u64 {
		self.length_of_segment
	}

	/// overwrites the length value in the footer with the given value. This can be useful, if you create an 'empty'
	/// footer (with length=0) and want to set the length value after reading the data from source to buffer.
	pub fn set_length_of_segment(&mut self, value: u64) {
		self.length_of_segment = value
	}

	/// adds an offset to the offset table of the SegmentFooter.
	pub fn add_offset(&mut self, offset: u64) {
		self.chunk_offsets.push(offset)
	}

	/// returns a reference of the offset table
	pub fn chunk_offsets(&self) -> &Vec<u64> {
		&self.chunk_offsets
	}
}

impl HeaderCoding for SegmentFooter {
	type Item = SegmentFooter;

	fn identifier() -> u32 {
		FOOTER_IDENTIFIER_SEGMENT_FOOTER
	}

	fn version(&self) -> u8 {
		self.version
	}

	fn encode_header(&self) -> Vec<u8> {
		let mut vec = Vec::new();

		vec.append(&mut self.version.encode_directly());
		vec.append(&mut self.length_of_segment.encode_directly());
		vec.append(&mut self.chunk_offsets.encode_directly());
		vec.append(&mut self.footer_offset.encode_directly());
		vec
	}

	fn decode_content(data: Vec<u8>) -> Result<SegmentFooter> {
		let mut cursor = Cursor::new(data);

		let footer_version = u8::decode_directly(&mut cursor)?;
		let length_of_segment = u64::decode_directly(&mut cursor)?;
		let chunk_offsets = Vec::<u64>::decode_directly(&mut cursor)?;
		let footer_offset = u64::decode_directly(&mut cursor)?;
		Ok(SegmentFooter::new(footer_version, length_of_segment, chunk_offsets, footer_offset))
	}
}