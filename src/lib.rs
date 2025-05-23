//! Advanced [`.npz`] file format support for [`ndarray`].
//!
//! # Accessing [`.npy`] Files
//!
//!   * See [`ndarray_npy`].
//!
//! # Accessing [`.npz`] Files
//!
//!   * Reading: [`NpzReader`]
//!   * Writing: [`NpzWriter`]
//!   * Immutable viewing (primarily for use with memory-mapped files):
//!       * [`NpzView`] providing an [`NpyView`] for each uncompressed [`.npy`] file within
//!         the archive
//!   * Mutable viewing (primarily for use with memory-mapped files):
//!       * [`NpzViewMut`] providing an [`NpyViewMut`] for each uncompressed [`.npy`] file within
//!         the archive
//!
//! [`.npy`]: https://numpy.org/doc/stable/reference/generated/numpy.lib.format.html
//! [`.npz`]: https://numpy.org/doc/stable/reference/generated/numpy.savez.html
//!
//! # Features
//!
//! Both features are enabled by default.
//!
//!   * `compressed`: Enables zip archives with *deflate* compression.
//!   * `num-complex-0_4`: Enables complex element types of crate `num-complex`.

#![forbid(unsafe_code)]
#![deny(
	missing_docs,
	rustdoc::broken_intra_doc_links,
	rustdoc::missing_crate_level_docs
)]
#![allow(clippy::tabs_in_doc_comments)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

// [`NpzReader`] and [`NpzWriter`] are derivative works of [`ndarray_npy`].

pub use ndarray;
pub use ndarray_npy;

use ndarray::{
	prelude::*,
	{Data, DataOwned},
};
use ndarray_npy::{
	ReadNpyError, ReadNpyExt, ReadableElement, ViewElement, ViewMutElement, ViewMutNpyExt,
	ViewNpyError, ViewNpyExt, WritableElement, WriteNpyError, WriteNpyExt,
};
use std::{
	collections::{BTreeMap, HashMap, HashSet},
	error::Error,
	fmt,
	io::{self, BufWriter, Cursor, Read, Seek, Write},
	ops::Range,
};
use zip::{
	result::ZipError,
	write::SimpleFileOptions,
	{CompressionMethod, ZipArchive, ZipWriter},
};

/// An error writing a `.npz` file.
#[derive(Debug)]
pub enum WriteNpzError {
	/// An error caused by the zip file.
	Zip(ZipError),
	/// An error caused by writing an inner `.npy` file.
	Npy(WriteNpyError),
}

impl Error for WriteNpzError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			WriteNpzError::Zip(err) => Some(err),
			WriteNpzError::Npy(err) => Some(err),
		}
	}
}

impl fmt::Display for WriteNpzError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			WriteNpzError::Zip(err) => write!(f, "zip file error: {err}"),
			WriteNpzError::Npy(err) => write!(f, "error writing npy file to npz archive: {err}"),
		}
	}
}

impl From<ZipError> for WriteNpzError {
	fn from(err: ZipError) -> WriteNpzError {
		WriteNpzError::Zip(err)
	}
}

impl From<WriteNpyError> for WriteNpzError {
	fn from(err: WriteNpyError) -> WriteNpzError {
		WriteNpzError::Npy(err)
	}
}

/// Writer for `.npz` files.
///
/// Note that the inner [`ZipWriter`] is wrapped in a [`BufWriter`] when
/// writing each array with [`.add_array()`](NpzWriter::add_array). If desired,
/// you could additionally buffer the innermost writer (e.g. the
/// [`File`](std::fs::File) when writing to a file) by wrapping it in a
/// [`BufWriter`]. This may be somewhat beneficial if the arrays are large and
/// have non-standard layouts but may decrease performance if the arrays have
/// standard or Fortran layout, so it's not recommended without testing to
/// compare.
///
/// # Example
///
/// ```no_run
/// use ndarray_npz::{
/// 	ndarray::{array, aview0, Array1, Array2},
/// 	NpzWriter,
/// };
/// use std::fs::File;
///
/// let mut npz = NpzWriter::new(File::create("arrays.npz")?);
/// let a: Array2<i32> = array![[1, 2, 3], [4, 5, 6]];
/// let b: Array1<i32> = array![7, 8, 9];
/// npz.add_array("a", &a)?;
/// npz.add_array("b", &b)?;
/// npz.add_array("c", &aview0(&10))?;
/// npz.finish()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub struct NpzWriter<W: Write + Seek> {
	zip: ZipWriter<W>,
	options: SimpleFileOptions,
}

impl<W: Write + Seek> NpzWriter<W> {
	/// Creates a new `.npz` file without compression. See [`numpy.savez`].
	///
	/// Ensures `.npy` files are 64-byte aligned for memory-mapping via [`NpzView`]/[`NpzViewMut`].
	///
	/// [`numpy.savez`]: https://numpy.org/doc/stable/reference/generated/numpy.savez.html
	#[must_use]
	pub fn new(writer: W) -> NpzWriter<W> {
		NpzWriter {
			zip: ZipWriter::new(writer),
			options: SimpleFileOptions::default()
				.with_alignment(64)
				.compression_method(CompressionMethod::Stored),
		}
	}

	/// Creates a new `.npz` file with compression. See [`numpy.savez_compressed`].
	///
	/// [`numpy.savez_compressed`]: https://numpy.org/doc/stable/reference/generated/numpy.savez_compressed.html
	#[cfg(feature = "compressed")]
	#[must_use]
	pub fn new_compressed(writer: W) -> NpzWriter<W> {
		NpzWriter {
			zip: ZipWriter::new(writer),
			options: SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
		}
	}

	/// Adds an array with the specified `name` to the `.npz` file.
	///
	/// To write a scalar value, create a zero-dimensional array using [`arr0`] or [`aview0`].
	///
	/// # Errors
	///
	/// Adding an array can fail with [`WriteNpyError`].
	pub fn add_array<N, S, D>(
		&mut self,
		name: N,
		array: &ArrayBase<S, D>,
	) -> Result<(), WriteNpzError>
	where
		N: Into<String>,
		S::Elem: WritableElement,
		S: Data,
		D: Dimension,
	{
		self.zip.start_file(name.into(), self.options)?;
		array.write_npy(BufWriter::new(&mut self.zip))?;
		Ok(())
	}

	/// Calls [`.finish()`](ZipWriter::finish) on the zip file and
	/// [`.flush()`](Write::flush) on the writer, and then returns the writer.
	///
	/// This finishes writing the remaining zip structures and flushes the
	/// writer. While dropping will automatically attempt to finish the zip
	/// file and (for writers that flush on drop, such as [`BufWriter`]) flush
	/// the writer, any errors that occur during drop will be silently ignored.
	/// So, it's necessary to call `.finish()` to properly handle errors.
	///
	/// # Errors
	///
	/// Finishing the zip archive can fail with [`ZipError`].
	pub fn finish(self) -> Result<W, WriteNpzError> {
		let mut writer = self.zip.finish()?;
		writer.flush().map_err(ZipError::from)?;
		Ok(writer)
	}
}

/// An error reading a `.npz` file.
#[derive(Debug)]
pub enum ReadNpzError {
	/// An error caused by the zip archive.
	Zip(ZipError),
	/// An error caused by reading an inner `.npy` file.
	Npy(ReadNpyError),
}

impl Error for ReadNpzError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ReadNpzError::Zip(err) => Some(err),
			ReadNpzError::Npy(err) => Some(err),
		}
	}
}

impl fmt::Display for ReadNpzError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			ReadNpzError::Zip(err) => write!(f, "zip file error: {err}"),
			ReadNpzError::Npy(err) => write!(f, "error reading npy file in npz archive: {err}"),
		}
	}
}

impl From<ZipError> for ReadNpzError {
	fn from(err: ZipError) -> ReadNpzError {
		ReadNpzError::Zip(err)
	}
}

impl From<ReadNpyError> for ReadNpzError {
	fn from(err: ReadNpyError) -> ReadNpzError {
		ReadNpzError::Npy(err)
	}
}

/// Reader for `.npz` files.
///
/// # Example
///
/// ```no_run
/// use ndarray_npz::{
/// 	ndarray::{Array1, Array2},
/// 	NpzReader,
/// };
/// use std::fs::File;
///
/// let mut npz = NpzReader::new(File::open("arrays.npz")?)?;
/// let a: Array2<i32> = npz.by_name("a")?;
/// let b: Array1<i32> = npz.by_name("b")?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub struct NpzReader<R: Read + Seek> {
	zip: ZipArchive<R>,
}

impl<R: Read + Seek> NpzReader<R> {
	/// Creates a new `.npz` file reader.
	///
	/// # Errors
	///
	/// Reading a zip archive can fail with [`ZipError`].
	pub fn new(reader: R) -> Result<NpzReader<R>, ReadNpzError> {
		Ok(NpzReader {
			zip: ZipArchive::new(reader)?,
		})
	}

	/// Returns `true` iff the `.npz` file doesn't contain any arrays.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.zip.len() == 0
	}

	/// Returns the number of arrays in the `.npz` file.
	#[must_use]
	pub fn len(&self) -> usize {
		self.zip.len()
	}

	/// Returns the names of all of the arrays in the file.
	///
	/// # Errors
	///
	/// Reading a zip archive can fail with [`ZipError`].
	pub fn names(&mut self) -> Result<Vec<String>, ReadNpzError> {
		Ok((0..self.zip.len())
			.map(|i| Ok(self.zip.by_index(i)?.name().to_owned()))
			.collect::<Result<_, ZipError>>()?)
	}

	/// Reads an array by name.
	///
	/// # Errors
	///
	/// Reading an array from an archive can fail with [`ReadNpyError`] or [`ZipError`].
	pub fn by_name<S, D>(&mut self, name: &str) -> Result<ArrayBase<S, D>, ReadNpzError>
	where
		S::Elem: ReadableElement,
		S: DataOwned,
		D: Dimension,
	{
		Ok(ArrayBase::<S, D>::read_npy(self.zip.by_name(name)?)?)
	}

	/// Reads an array by index in the `.npz` file.
	///
	/// # Errors
	///
	/// Reading an array from an archive can fail with [`ReadNpyError`] or [`ZipError`].
	pub fn by_index<S, D>(&mut self, index: usize) -> Result<ArrayBase<S, D>, ReadNpzError>
	where
		S::Elem: ReadableElement,
		S: DataOwned,
		D: Dimension,
	{
		Ok(ArrayBase::<S, D>::read_npy(self.zip.by_index(index)?)?)
	}
}

/// An error viewing a `.npz` file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ViewNpzError {
	/// An error caused by the zip archive.
	Zip(ZipError),
	/// An error caused by viewing an inner `.npy` file.
	Npy(ViewNpyError),
	/// A mutable `.npy` file view has already been moved out of its `.npz` file view.
	MovedNpyViewMut,
	/// Directories cannot be viewed.
	Directory,
	/// Compressed files cannot be viewed.
	CompressedFile,
	/// Encrypted files cannot be viewed.
	EncryptedFile,
}

impl Error for ViewNpzError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ViewNpzError::Zip(err) => Some(err),
			ViewNpzError::Npy(err) => Some(err),
			ViewNpzError::MovedNpyViewMut
			| ViewNpzError::Directory
			| ViewNpzError::CompressedFile
			| ViewNpzError::EncryptedFile => None,
		}
	}
}

impl fmt::Display for ViewNpzError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			ViewNpzError::Zip(err) => write!(f, "zip file error: {err}"),
			ViewNpzError::Npy(err) => write!(f, "error viewing npy file in npz archive: {err}"),
			ViewNpzError::MovedNpyViewMut => write!(
				f,
				"mutable npy file view already moved out of npz file view"
			),
			ViewNpzError::Directory => write!(f, "directories cannot be viewed"),
			ViewNpzError::CompressedFile => write!(f, "compressed files cannot be viewed"),
			ViewNpzError::EncryptedFile => write!(f, "encrypted files cannot be viewed"),
		}
	}
}

impl From<ZipError> for ViewNpzError {
	fn from(err: ZipError) -> ViewNpzError {
		ViewNpzError::Zip(err)
	}
}

impl From<ViewNpyError> for ViewNpzError {
	fn from(err: ViewNpyError) -> ViewNpzError {
		ViewNpzError::Npy(err)
	}
}

/// Immutable view for memory-mapped `.npz` files.
///
/// The primary use-case for this is viewing `.npy` files within a memory-mapped
/// `.npz` archive.
///
/// # Notes
///
/// - For types for which not all bit patterns are valid, such as `bool`, the
///   implementation iterates over all of the elements when creating the view
///   to ensure they have a valid bit pattern.
/// - The data in the buffer containing an `.npz` archive must be properly
///   aligned for its `.npy` file with the maximum alignment requirement for its
///   element type. Typically, this should not be a concern for memory-mapped
///   files (unless an option like `MAP_FIXED` is used), since memory mappings
///   are usually aligned to a page boundary.
/// - The `.npy` files within the `.npz` archive must be properly aligned for
///   their element type. Archives not created by this crate can be aligned with
///   the help of the CLI tool [`rezip`] as in `rezip in.npz -o out.npz`.
///
/// [`rezip`]: https://crates.io/crates/rezip
///
/// # Example
///
/// This is an example of opening an immutably memory-mapped `.npz` archive as
/// an [`NpzView`] providing an [`NpyView`] for each non-compressed and
/// non-encrypted `.npy` file within the archive which can be accessed via
/// [`NpyView::view`] as immutable [`ArrayView`].
///
/// This example uses the [`memmap2`](https://crates.io/crates/memmap2) crate
/// because that appears to be the best-maintained memory-mapping crate at the
/// moment, but [`Self::new`] takes a `&mut [u8]` instead of a file so that you
/// can use the memory-mapping crate you're most comfortable with.
///
/// ```
/// # if !cfg!(miri) { // Miri doesn't support mmap.
/// use std::fs::OpenOptions;
///
/// use memmap2::MmapOptions;
/// use ndarray::Ix1;
/// use ndarray_npz::{NpzView, ViewNpzError};
///
/// // Open `.npz` archive of non-compressed and non-encrypted `.npy` files in
/// // native endian.
/// #[cfg(target_endian = "little")]
/// let file = OpenOptions::new()
/// 	.read(true)
/// 	.open("tests/examples_little_endian_64_byte_aligned.npz")
/// 	.unwrap();
/// #[cfg(target_endian = "big")]
/// let file = OpenOptions::new()
/// 	.read(true)
/// 	.open("tests/examples_big_endian_64_byte_aligned.npz")
/// 	.unwrap();
/// // Memory-map `.npz` archive of 64-byte aligned `.npy` files.
/// let mmap = unsafe { MmapOptions::new().map(&file).unwrap() };
/// let npz = NpzView::new(&mmap)?;
/// // List non-compressed and non-encrypted files only.
/// for npy in npz.names() {
/// 	println!("{}", npy);
/// }
/// // Get immutable `.npy` views.
/// let mut x_npy_view = npz.by_name("i64.npy")?;
/// let mut y_npy_view = npz.by_name("f64.npy")?;
/// // Optionally verify CRC-32 checksums.
/// x_npy_view.verify()?;
/// y_npy_view.verify()?;
/// // Get and print immutable `ArrayView`s.
/// let x_array_view = x_npy_view.view::<i64, Ix1>()?;
/// let y_array_view = y_npy_view.view::<f64, Ix1>()?;
/// println!("{}", x_array_view);
/// println!("{}", y_array_view);
/// # }
/// # Ok::<(), ndarray_npz::ViewNpzError>(())
/// ```
#[derive(Debug, Clone)]
pub struct NpzView<'a> {
	files: HashMap<usize, NpyView<'a>>,
	names: HashMap<String, usize>,
	directory_names: HashSet<String>,
	compressed_names: HashSet<String>,
	encrypted_names: HashSet<String>,
}

impl<'a> NpzView<'a> {
	/// Creates a new immutable view of a memory-mapped `.npz` file.
	///
	/// # Errors
	///
	/// Viewing an archive can fail with [`ZipError`].
	pub fn new(bytes: &'a [u8]) -> Result<Self, ViewNpzError> {
		let mut zip = ZipArchive::new(Cursor::new(bytes))?;
		let mut archive = Self {
			files: HashMap::new(),
			names: HashMap::new(),
			directory_names: HashSet::new(),
			compressed_names: HashSet::new(),
			encrypted_names: zip.file_names().map(From::from).collect(),
		};
		// Initially assume all files to be encrypted.
		let mut index = 0;
		for zip_index in 0..zip.len() {
			// Skip encrypted files.
			let file = match zip.by_index(zip_index) {
				Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)) => continue,
				Err(err) => return Err(err.into()),
				Ok(file) => file,
			};
			// File name of non-encrypted file.
			let name = file.name().to_string();
			// Remove file name from encrypted files.
			archive.encrypted_names.remove(&name);
			// Skip directories and compressed files.
			if file.is_dir() {
				archive.directory_names.insert(name);
				continue;
			}
			if file.compression() != CompressionMethod::Stored {
				archive.compressed_names.insert(name);
				continue;
			}
			// Store file index by file names.
			archive.names.insert(name, index);
			let file = NpyView {
				data: slice_at(bytes, file.data_start(), 0..file.size())?,
				central_crc32: slice_at(bytes, file.central_header_start(), 16..20)
					.map(as_array_ref)?,
				status: ChecksumStatus::default(),
			};
			// Store file view by file index.
			archive.files.insert(index, file);
			// Increment index of non-compressed and non-encrypted files.
			index += 1;
		}
		Ok(archive)
	}

	/// Returns `true` iff the `.npz` file doesn't contain any viewable arrays.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.names.is_empty()
	}

	/// Returns the number of viewable arrays in the `.npz` file.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	#[must_use]
	pub fn len(&self) -> usize {
		self.names.len()
	}

	/// Returns the names of all of the viewable arrays in the `.npz` file.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	pub fn names(&self) -> impl Iterator<Item = &str> {
		self.names.keys().map(String::as_str)
	}
	/// Returns the names of all of the directories in the `.npz` file.
	pub fn directory_names(&self) -> impl Iterator<Item = &str> {
		self.directory_names.iter().map(String::as_str)
	}
	/// Returns the names of all of the compressed files in the `.npz` file.
	pub fn compressed_names(&self) -> impl Iterator<Item = &str> {
		self.compressed_names.iter().map(String::as_str)
	}
	/// Returns the names of all of the encrypted files in the `.npz` file.
	pub fn encrypted_names(&self) -> impl Iterator<Item = &str> {
		self.encrypted_names.iter().map(String::as_str)
	}

	/// Returns an immutable `.npy` file view by name.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`]. Trying to view a directory,
	/// compressed file, or encrypted file, fails with [`ViewNpzError::Directory`],
	/// [`ViewNpzError::CompressedFile`], or [`ViewNpzError::CompressedFile`]. Fails with
	/// [`ZipError::FileNotFound`] if the `name` is not found.
	pub fn by_name(&self, name: &str) -> Result<NpyView<'a>, ViewNpzError> {
		self.by_index(self.names.get(name).copied().ok_or_else(|| {
			if self.directory_names.contains(name) {
				ViewNpzError::Directory
			} else if self.compressed_names.contains(name) {
				ViewNpzError::CompressedFile
			} else if self.encrypted_names.contains(name) {
				ViewNpzError::EncryptedFile
			} else {
				ZipError::FileNotFound.into()
			}
		})?)
	}

	/// Returns an immutable `.npy` file view by index in `0..len()`.
	///
	/// The index **does not** necessarily correspond to the index of the zip archive as
	/// directories, compressed files, and encrypted files are skipped.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`]. Fails with [`ZipError::FileNotFound`]
	/// if the `index` is not found.
	pub fn by_index(&self, index: usize) -> Result<NpyView<'a>, ViewNpzError> {
		self.files
			.get(&index)
			.copied()
			.ok_or_else(|| ZipError::FileNotFound.into())
	}
}

/// Immutable view of memory-mapped `.npy` files within an `.npz` file.
///
/// Does **not** automatically [verify](`Self::verify`) CRC-32 checksum.
#[derive(Debug, Clone, Copy)]
pub struct NpyView<'a> {
	data: &'a [u8],
	central_crc32: &'a [u8; 4],
	status: ChecksumStatus,
}

impl NpyView<'_> {
	/// CRC-32 checksum status.
	#[must_use]
	pub fn status(&self) -> ChecksumStatus {
		self.status
	}
	/// Verifies and returns CRC-32 checksum by reading the whole array.
	///
	/// Changes checksum [`status`](`Self::status()`) to [`Outdated`](`ChecksumStatus::Outdated`)
	/// if invalid or to [`Correct`](`ChecksumStatus::Correct`) if valid.
	///
	/// # Errors
	///
	/// Fails with [`ZipError::Io`] if the checksum is invalid.
	pub fn verify(&mut self) -> Result<u32, ViewNpzError> {
		self.status = ChecksumStatus::Outdated;
		// Like the `zip` crate, verify only against central CRC-32.
		let crc32 = crc32_verify(self.data, *self.central_crc32)?;
		self.status = ChecksumStatus::Correct;
		Ok(crc32)
	}

	/// Returns an immutable view of a memory-mapped `.npy` file.
	///
	/// Iterates over `bool` array to ensure `0x00`/`0x01` values.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`].
	pub fn view<A, D>(&self) -> Result<ArrayView<A, D>, ViewNpzError>
	where
		A: ViewElement,
		D: Dimension,
	{
		Ok(ArrayView::view_npy(self.data)?)
	}
}

/// Mutable view for memory-mapped `.npz` files.
///
/// The primary use-case for this is modifying `.npy` files within a
/// memory-mapped `.npz` archive. Modifying the elements in the view will modify
/// the file. Modifying the shape/strides of the view will *not* modify the
/// shape/strides of the array in the file.
///
/// # Notes
///
/// - For types for which not all bit patterns are valid, such as `bool`, the
///   implementation iterates over all of the elements when creating the view
///   to ensure they have a valid bit pattern.
/// - The data in the buffer containing an `.npz` archive must be properly
///   aligned for its `.npy` file with the maximum alignment requirement for its
///   element type. Typically, this should not be a concern for memory-mapped
///   files (unless an option like `MAP_FIXED` is used), since memory mappings
///   are usually aligned to a page boundary.
/// - The `.npy` files within the `.npz` archive must be properly aligned for
///   their element type. Archives not created by this crate can be aligned with
///   the help of the CLI tool [`rezip`] as in `rezip in.npz -o out.npz`.
///
/// [`rezip`]: https://crates.io/crates/rezip
///
/// # Example
///
/// This is an example of opening a mutably memory-mapped `.npz` archive as an
/// [`NpzViewMut`] providing an [`NpyViewMut`] for each non-compressed and
/// non-encrypted `.npy` file within the archive which can be accessed via
/// [`NpyViewMut::view`] as immutable [`ArrayView`] or via
/// [`NpyViewMut::view_mut`] as mutable [`ArrayViewMut`]. Changes to the data in
/// the view will modify the underlying file within the archive.
///
/// This example uses the [`memmap2`](https://crates.io/crates/memmap2) crate
/// because that appears to be the best-maintained memory-mapping crate at the
/// moment, but [`Self::new`] takes a `&mut [u8]` instead of a file so that you
/// can use the memory-mapping crate you're most comfortable with.
///
/// # Example
///
/// ```
/// # if !cfg!(miri) { // Miri doesn't support mmap.
/// use std::fs::OpenOptions;
///
/// use memmap2::MmapOptions;
/// use ndarray::Ix1;
/// use ndarray_npz::{NpzViewMut, ViewNpzError};
///
/// // Open `.npz` archive of non-compressed and non-encrypted `.npy` files in
/// // native endian.
/// #[cfg(target_endian = "little")]
/// let mut file = OpenOptions::new()
/// 	.read(true)
/// 	.write(true)
/// 	.open("tests/examples_little_endian_64_byte_aligned.npz")
/// 	.unwrap();
/// #[cfg(target_endian = "big")]
/// let mut file = OpenOptions::new()
/// 	.read(true)
/// 	.write(true)
/// 	.open("tests/examples_big_endian_64_byte_aligned.npz")
/// 	.unwrap();
/// // Memory-map `.npz` archive of 64-byte aligned `.npy` files.
/// let mut mmap = unsafe { MmapOptions::new().map_mut(&file).unwrap() };
/// let mut npz = NpzViewMut::new(&mut mmap)?;
/// // List non-compressed and non-encrypted files only.
/// for npy in npz.names() {
/// 	println!("{}", npy);
/// }
/// // Get mutable `.npy` views of both arrays at the same time.
/// let mut x_npy_view_mut = npz.by_name("i64.npy")?;
/// let mut y_npy_view_mut = npz.by_name("f64.npy")?;
/// // Optionally verify CRC-32 checksums.
/// x_npy_view_mut.verify()?;
/// y_npy_view_mut.verify()?;
/// // Get and print mutable `ArrayViewMut`s.
/// let x_array_view_mut = x_npy_view_mut.view_mut::<i64, Ix1>()?;
/// let y_array_view_mut = y_npy_view_mut.view_mut::<f64, Ix1>()?;
/// println!("{}", x_array_view_mut);
/// println!("{}", y_array_view_mut);
/// // Update CRC-32 checksums after changes. Automatically updated on `drop()`
/// // if outdated.
/// x_npy_view_mut.update();
/// y_npy_view_mut.update();
/// # }
/// # Ok::<(), ndarray_npz::ViewNpzError>(())
/// ```
#[derive(Debug)]
pub struct NpzViewMut<'a> {
	files: HashMap<usize, NpyViewMut<'a>>,
	names: HashMap<String, usize>,
	directory_names: HashSet<String>,
	compressed_names: HashSet<String>,
	encrypted_names: HashSet<String>,
}

impl<'a> NpzViewMut<'a> {
	/// Creates a new mutable view of a memory-mapped `.npz` file.
	///
	/// # Errors
	///
	/// Viewing an archive can fail with [`ZipError`].
	pub fn new(mut bytes: &'a mut [u8]) -> Result<Self, ViewNpzError> {
		let mut zip = ZipArchive::new(Cursor::new(&bytes))?;
		let mut archive = Self {
			files: HashMap::new(),
			names: HashMap::new(),
			directory_names: HashSet::new(),
			compressed_names: HashSet::new(),
			encrypted_names: zip.file_names().map(From::from).collect(),
		};
		// Initially assume all files to be encrypted.
		let mut ranges = HashMap::new();
		let mut splits = BTreeMap::new();
		let mut index = 0;
		for zip_index in 0..zip.len() {
			// Skip encrypted files.
			let file = match zip.by_index(zip_index) {
				Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED)) => continue,
				Err(err) => return Err(err.into()),
				Ok(file) => file,
			};
			// File name of non-encrypted file.
			let name = file.name().to_string();
			// Remove file name from encrypted files.
			archive.encrypted_names.remove(&name);
			// Skip directories and compressed files.
			if file.is_dir() {
				archive.directory_names.insert(name);
				continue;
			}
			if file.compression() != CompressionMethod::Stored {
				archive.compressed_names.insert(name);
				continue;
			}
			// Skip directories and compressed files.
			if file.is_dir() || file.compression() != CompressionMethod::Stored {
				continue;
			}
			// Store file index by file names.
			archive.names.insert(name, index);
			// Get data range.
			let data_range = range_at(file.data_start(), 0..file.size())?;
			// Get central general purpose bit flag range.
			let central_flag_range = range_at(file.central_header_start(), 8..10)?;
			// Parse central general purpose bit flag range.
			let central_flag = u16_at(bytes, central_flag_range);
			// Get central CRC-32 range.
			let central_crc32_range = range_at(file.central_header_start(), 16..20)?;
			// Whether local CRC-32 is located in header or data descriptor.
			let use_data_descriptor = central_flag & (1 << 3) != 0;
			// Get local CRC-32 range.
			let crc32_range = if use_data_descriptor {
				// Get local CRC-32 range in data descriptor.
				let crc32_range = range_at(data_range.end, 0..4)?;
				// Parse local CRC-32.
				let crc32 = u32_at(bytes, crc32_range.clone());
				// Whether local CRC-32 equals optional data descriptor signature.
				if crc32 == 0x0807_4b50 {
					// Parse central CRC-32.
					let central_crc32 = u32_at(bytes, central_crc32_range.clone());
					// Whether CRC-32 coincides with data descriptor signature.
					if crc32 == central_crc32 {
						return Err(ZipError::InvalidArchive(
							"Ambiguous CRC-32 location in data descriptor".into(),
						)
						.into());
					}
					// Skip data descriptor signature and get local CRC-32 range in data descriptor.
					range_at(data_range.end, 4..8)?
				} else {
					crc32_range
				}
			} else {
				// Get local CRC-32 range in header.
				range_at(file.header_start(), 14..18)?
			};
			// Sort ranges by their starts.
			splits.insert(crc32_range.start, crc32_range.end);
			splits.insert(data_range.start, data_range.end);
			splits.insert(central_crc32_range.start, central_crc32_range.end);
			// Store ranges by file index.
			ranges.insert(index, (data_range, crc32_range, central_crc32_range));
			// Increment index of non-compressed and non-encrypted files.
			index += 1;
		}
		// Split and store borrows by their range starts.
		let mut offset = 0;
		let mut slices = HashMap::new();
		for (&start, &end) in &splits {
			// Split off leading bytes.
			let mid = start
				.checked_sub(offset)
				.ok_or(ZipError::InvalidArchive("Overlapping ranges".into()))?;
			if mid > bytes.len() {
				return Err(ZipError::InvalidArchive("Offset exceeds EOF".into()).into());
			}
			let (slice, remaining_bytes) = bytes.split_at_mut(mid);
			offset += slice.len();
			// Split off leading borrow of interest. Cannot underflow since `start < end`.
			let mid = end - offset;
			if mid > remaining_bytes.len() {
				return Err(ZipError::InvalidArchive("Length exceeds EOF".into()).into());
			}
			let (slice, remaining_bytes) = remaining_bytes.split_at_mut(mid);
			offset += slice.len();
			// Store borrow by its range start.
			slices.insert(start, slice);
			// Store remaining bytes.
			bytes = remaining_bytes;
		}
		// Collect split borrows as file views.
		for (&index, (data_range, crc32_range, central_crc32_range)) in &ranges {
			let ambiguous_offset = || ZipError::InvalidArchive("Ambiguous offsets".into());
			let file = NpyViewMut {
				data: slices
					.remove(&data_range.start)
					.ok_or_else(ambiguous_offset)?,
				crc32: slices
					.remove(&crc32_range.start)
					.map(as_array_mut)
					.ok_or_else(ambiguous_offset)?,
				central_crc32: slices
					.remove(&central_crc32_range.start)
					.map(as_array_mut)
					.ok_or_else(ambiguous_offset)?,
				status: ChecksumStatus::default(),
			};
			archive.files.insert(index, file);
		}
		Ok(archive)
	}

	/// Returns `true` iff the `.npz` file doesn't contain any viewable arrays.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.names.is_empty()
	}

	/// Returns the number of viewable arrays in the `.npz` file.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	#[must_use]
	pub fn len(&self) -> usize {
		self.names.len()
	}

	/// Returns the names of all of the viewable arrays in the `.npz` file.
	///
	/// Viewable arrays are neither directories, nor compressed, nor encrypted.
	pub fn names(&self) -> impl Iterator<Item = &str> {
		self.names.keys().map(String::as_str)
	}
	/// Returns the names of all of the directories in the `.npz` file.
	pub fn directory_names(&self) -> impl Iterator<Item = &str> {
		self.directory_names.iter().map(String::as_str)
	}
	/// Returns the names of all of the compressed files in the `.npz` file.
	pub fn compressed_names(&self) -> impl Iterator<Item = &str> {
		self.compressed_names.iter().map(String::as_str)
	}
	/// Returns the names of all of the encrypted files in the `.npz` file.
	pub fn encrypted_names(&self) -> impl Iterator<Item = &str> {
		self.encrypted_names.iter().map(String::as_str)
	}

	/// Moves a mutable `.npy` file view by name out of the `.npz` file view.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`]. Trying to view a directory,
	/// compressed file, or encrypted file, fails with [`ViewNpzError::Directory`],
	/// [`ViewNpzError::CompressedFile`], or [`ViewNpzError::CompressedFile`]. Fails with
	/// [`ZipError::FileNotFound`] if the `name` is not found.
	pub fn by_name(&mut self, name: &str) -> Result<NpyViewMut<'a>, ViewNpzError> {
		self.by_index(self.names.get(name).copied().ok_or_else(|| {
			if self.directory_names.contains(name) {
				ViewNpzError::Directory
			} else if self.compressed_names.contains(name) {
				ViewNpzError::CompressedFile
			} else if self.encrypted_names.contains(name) {
				ViewNpzError::EncryptedFile
			} else {
				ZipError::FileNotFound.into()
			}
		})?)
	}

	/// Moves a mutable `.npy` file view by index in `0..len()` out of the `.npz` file view.
	///
	/// The index **does not** necessarily correspond to the index of the zip archive as
	/// directories, compressed files, and encrypted files are skipped.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`]. Fails with [`ZipError::FileNotFound`]
	/// if the `index` is not found. Fails with [`ViewNpzError::MovedNpyViewMut`] if the mutable
	/// `.npy` file view has already been moved out of the `.npz` file view.
	pub fn by_index(&mut self, index: usize) -> Result<NpyViewMut<'a>, ViewNpzError> {
		if index > self.names.len() {
			Err(ZipError::FileNotFound.into())
		} else {
			self.files
				.remove(&index)
				.ok_or(ViewNpzError::MovedNpyViewMut)
		}
	}
}

/// Mutable view of memory-mapped `.npy` files within an `.npz` file.
///
/// Does **not** automatically [verify](`Self::verify`) the CRC-32 checksum but **does**
/// [update](`Self::update`) it on [`Drop::drop`] if [`view_mut`](`Self::view_mut`) has been invoked
/// and the checksum has not manually been updated by invoking [`update`](`Self::update`).
#[derive(Debug)]
pub struct NpyViewMut<'a> {
	data: &'a mut [u8],
	crc32: &'a mut [u8; 4],
	central_crc32: &'a mut [u8; 4],
	status: ChecksumStatus,
}

impl NpyViewMut<'_> {
	/// CRC-32 checksum status.
	#[must_use]
	pub fn status(&self) -> ChecksumStatus {
		self.status
	}
	/// Verifies and returns CRC-32 checksum by reading the whole array.
	///
	/// Changes checksum [`status`](`Self::status()`) to [`Outdated`](`ChecksumStatus::Outdated`)
	/// if invalid or to [`Correct`](`ChecksumStatus::Correct`) if valid.
	///
	/// # Errors
	///
	/// Fails with [`ZipError::Io`] if the checksum is invalid.
	pub fn verify(&mut self) -> Result<u32, ViewNpzError> {
		self.status = ChecksumStatus::Outdated;
		// Like the `zip` crate, verify only against central CRC-32.
		let crc32 = crc32_verify(self.data, *self.central_crc32)?;
		self.status = ChecksumStatus::Correct;
		Ok(crc32)
	}
	/// Updates and returns CRC-32 checksum by reading the whole array.
	///
	/// Changes checksum [`status`](`Self::status()`) to [`Correct`](`ChecksumStatus::Correct`).
	///
	/// Automatically updated on [`Drop::drop`] iff checksum [`status`](`Self::status()`) is
	/// [`Outdated`](`ChecksumStatus::Outdated`).
	pub fn update(&mut self) -> u32 {
		self.status = ChecksumStatus::Correct;
		let crc32 = crc32_update(self.data);
		*self.central_crc32 = crc32.to_le_bytes();
		*self.crc32 = *self.central_crc32;
		crc32
	}

	/// Returns an immutable view of a memory-mapped `.npy` file.
	///
	/// Iterates over `bool` array to ensure `0x00`/`0x01` values.
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`].
	pub fn view<A, D>(&self) -> Result<ArrayView<A, D>, ViewNpzError>
	where
		A: ViewElement,
		D: Dimension,
	{
		Ok(ArrayView::<A, D>::view_npy(self.data)?)
	}
	/// Returns a mutable view of a memory-mapped `.npy` file.
	///
	/// Iterates over `bool` array to ensure `0x00`/`0x01` values.
	///
	/// Changes checksum [`status`](`Self::status()`) to [`Outdated`](`ChecksumStatus::Outdated`).
	///
	/// # Errors
	///
	/// Viewing an `.npy` file can fail with [`ViewNpyError`].
	pub fn view_mut<A, D>(&mut self) -> Result<ArrayViewMut<A, D>, ViewNpzError>
	where
		A: ViewMutElement,
		D: Dimension,
	{
		self.status = ChecksumStatus::Outdated;
		Ok(ArrayViewMut::<A, D>::view_mut_npy(self.data)?)
	}
}

impl Drop for NpyViewMut<'_> {
	fn drop(&mut self) {
		if self.status == ChecksumStatus::Outdated {
			self.update();
		}
	}
}

/// Checksum status of an [`NpyView`] or [`NpyViewMut`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumStatus {
	/// The checksum has not been computed and the data has not changed.
	Unverified,
	/// The checksum is correct and the data has not changed.
	Correct,
	/// The data may have changed.
	Outdated,
}

impl Default for ChecksumStatus {
	fn default() -> Self {
		Self::Unverified
	}
}

fn crc32_verify(bytes: &[u8], crc32: [u8; 4]) -> Result<u32, ZipError> {
	let crc32 = u32::from_le_bytes(crc32);
	if crc32_update(bytes) == crc32 {
		Ok(crc32)
	} else {
		Err(ZipError::Io(io::Error::other("Invalid checksum")))
	}
}

#[must_use]
fn crc32_update(bytes: &[u8]) -> u32 {
	let mut hasher = crc32fast::Hasher::new();
	hasher.update(bytes);
	hasher.finalize()
}

fn range_at<T>(index: T, range: Range<T>) -> Result<Range<usize>, ZipError>
where
	T: TryInto<usize> + Copy,
{
	index
		.try_into()
		.ok()
		.and_then(|index| {
			let start = range.start.try_into().ok()?.checked_add(index)?;
			let end = range.end.try_into().ok()?.checked_add(index)?;
			Some(start..end)
		})
		.ok_or(ZipError::InvalidArchive("Range overflow".into()))
}

fn slice_at<T>(bytes: &[u8], index: T, range: Range<T>) -> Result<&[u8], ZipError>
where
	T: TryInto<usize> + Copy,
{
	let range = range_at(index, range)?;
	bytes
		.get(range)
		.ok_or(ZipError::InvalidArchive("Range exceeds EOF".into()))
}

#[must_use]
fn u16_at(bytes: &[u8], range: Range<usize>) -> u16 {
	u16::from_le_bytes(bytes.get(range).unwrap().try_into().unwrap())
}

#[must_use]
fn u32_at(bytes: &[u8], range: Range<usize>) -> u32 {
	u32::from_le_bytes(bytes.get(range).unwrap().try_into().unwrap())
}

#[must_use]
fn as_array_ref(slice: &[u8]) -> &[u8; 4] {
	slice.try_into().unwrap()
}

#[must_use]
fn as_array_mut(slice: &mut [u8]) -> &mut [u8; 4] {
	slice.try_into().unwrap()
}
