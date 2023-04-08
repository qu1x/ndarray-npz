use ndarray::prelude::*;

#[test]
fn npz_view_mut() {
	use aligned_vec::AVec;
	use ndarray_npz::{NpzView, NpzViewMut, NpzWriter};
	use std::{fs::read, io::Cursor};

	// Signature of local header.
	let header = [0x50, 0x4b, 0x03, 0x04];
	// Optional signature of data descriptor.
	let data_descriptor = [0x50, 0x4b, 0x07, 0x08];
	// Parse local header CRC-32.
	let header_crc32 = |buffer: &[u8], offset: usize| {
		u32::from_le_bytes((&buffer[offset + 14..offset + 18]).try_into().unwrap())
	};
	// Parse data descriptor CRC-32.
	let data_descriptor_crc32 = |buffer: &[u8], offset: usize| {
		u32::from_le_bytes((&buffer[offset + 4..offset + 8]).try_into().unwrap())
	};
	// In-memory buffer.
	let mut buffer = Vec::<u8>::new();
	// Create an `.npz` archive with zeroed `.npy` arrays.
	{
		let mut npz = NpzWriter::new(Cursor::new(&mut buffer));
		npz.add_array("x.npy", &Array1::<f64>::zeros(5)).unwrap();
		npz.add_array("y.npy", &Array1::<f64>::zeros(7)).unwrap();
		npz.add_array("z.npy", &Array1::<f64>::zeros(9)).unwrap();
	}
	let mut buffer = AVec::<u8>::from_slice(64, &buffer);
	// Confirm all three example arrays have local headers.
	let offsets = find_subsequence(&buffer, &header);
	assert_eq!(&offsets, &[0, 232, 504]);
	assert_eq!(&buffer[offsets[0]..offsets[0] + 4], &header);
	assert_eq!(&buffer[offsets[1]..offsets[1] + 4], &header);
	assert_eq!(&buffer[offsets[2]..offsets[2] + 4], &header);
	// Parse local header CRC-32 of arrays.
	let x_crc32 = header_crc32(&buffer, offsets[0]);
	let y_crc32 = header_crc32(&buffer, offsets[1]);
	let z_crc32 = header_crc32(&buffer, offsets[2]);
	// Create mutable view of the `.npz` archive and modify data of arrays.
	let (x_central_crc32, y_central_crc32, z_central_crc32) = {
		let mut npz = NpzViewMut::new(&mut buffer).unwrap();
		// Modify `x.npy`.
		let mut x_npy_view_mut = npz.by_name("x.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		let x_central_crc32 = x_npy_view_mut.verify().unwrap();
		// Verify that local and central header CRC-32 equal.
		assert_eq!(x_crc32, x_central_crc32);
		let mut x_array_view_mut = x_npy_view_mut.view_mut::<f64, Ix1>().unwrap();
		x_array_view_mut[0] = 1.0;
		x_array_view_mut[3] = 8.0;
		x_array_view_mut[4] = 7.0;
		x_npy_view_mut.verify().unwrap_err();
		// Compute and write updated CRC-32 to local and central header.
		let x_central_crc32 = x_npy_view_mut.update();
		// Keep `y.npy` untouched.
		let mut y_npy_view_mut = npz.by_name("y.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		let y_central_crc32 = y_npy_view_mut.verify().unwrap();
		// Verify that local and central header CRC-32 equal.
		assert_eq!(y_crc32, y_central_crc32);
		// Modify `z.npy`.
		let mut z_npy_view_mut = npz.by_name("z.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		let z_central_crc32 = z_npy_view_mut.verify().unwrap();
		// Verify that local and central header CRC-32 equal.
		assert_eq!(z_crc32, z_central_crc32);
		let mut z_array_view_mut = z_npy_view_mut.view_mut::<f64, Ix1>().unwrap();
		z_array_view_mut[0] = 3.0;
		z_array_view_mut[2] = 3.5;
		z_array_view_mut[6] = 9.0;
		z_npy_view_mut.verify().unwrap_err();
		// Compute and write updated CRC-32 to local and central header.
		let z_central_crc32 = z_npy_view_mut.update();
		(x_central_crc32, y_central_crc32, z_central_crc32)
	};
	// Parse updated local header CRC-32 of arrays.
	let x_crc32 = header_crc32(&buffer, offsets[0]);
	let y_crc32 = header_crc32(&buffer, offsets[1]);
	let z_crc32 = header_crc32(&buffer, offsets[2]);
	// Verify that updated local header and central CRC-32 equal.
	assert_eq!(x_crc32, x_central_crc32);
	assert_eq!(y_crc32, y_central_crc32);
	assert_eq!(z_crc32, z_central_crc32);
	// Create immutable view of the `.npz` archive and verify data and CRC-32 of arrays.
	{
		let npz = NpzView::new(&buffer).unwrap();
		// Verify `x.npy`.
		let mut x_npy_view = npz.by_name("x.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		x_npy_view.verify().unwrap();
		let x_array_view = x_npy_view.view::<f64, Ix1>().unwrap();
		assert_eq!(x_array_view, ArrayView1::from(&[1.0, 0.0, 0.0, 8.0, 7.0]));
		// Verify `y.npy`.
		let mut y_npy_view = npz.by_name("y.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		y_npy_view.verify().unwrap();
		let y_array_view = y_npy_view.view::<f64, Ix1>().unwrap();
		assert_eq!(y_array_view, ArrayView1::from(&[0.0; 7]));
		// Verify `z.npy`.
		let mut z_npy_view = npz.by_name("z.npy").unwrap();
		// Verify that central header CRC-32 is correct.
		z_npy_view.verify().unwrap();
		let z_array_view = z_npy_view.view::<f64, Ix1>().unwrap();
		assert_eq!(
			z_array_view,
			ArrayView1::from(&[3.0, 0.0, 3.5, 0.0, 0.0, 0.0, 9.0, 0.0, 0.0])
		);
	}
	{
		// Read archive with data descriptors into buffer.
		let mut buffer = read("tests/examples_data_descriptor.npz").unwrap();
		// Confirm all three example arrays have data descriptors.
		let offsets = find_subsequence(&buffer, &data_descriptor);
		assert_eq!(&offsets, &[194, 404, 614]);
		assert_eq!(&buffer[offsets[0]..offsets[0] + 4], &data_descriptor);
		assert_eq!(&buffer[offsets[1]..offsets[1] + 4], &data_descriptor);
		assert_eq!(&buffer[offsets[2]..offsets[2] + 4], &data_descriptor);
		// Parse data descriptor CRC-32 of first array.
		let crc32 = data_descriptor_crc32(&buffer, offsets[0]);
		let central_crc32 = {
			let mut npz = NpzViewMut::new(&mut buffer).unwrap();
			let mut x_npy_view_mut = npz.by_name("b8.npy").unwrap();
			// Verify that central header CRC-32 is correct.
			let central_crc32 = x_npy_view_mut.verify().unwrap();
			// Verify that data descriptor and central header CRC-32 equal.
			assert_eq!(crc32, central_crc32);
			// Modify array.
			let mut x_array_view_mut = x_npy_view_mut.view_mut::<bool, Ix1>().unwrap();
			x_array_view_mut[0] = false;
			x_array_view_mut[1] = true;
			// Ensure modification actually changed data and that central header CRC-32 is outdated.
			x_npy_view_mut.verify().unwrap_err();
			// Compute and write updated CRC-32 to data descriptor and central header.
			x_npy_view_mut.update()
		};
		// Parse updated data descriptor CRC-32 of first array.
		let crc32 = data_descriptor_crc32(&buffer, offsets[0]);
		// Verify that updated data descriptor and central header CRC-32 equal.
		assert_eq!(crc32, central_crc32);
	}
}

fn find_subsequence<T>(haystack: &[T], needle: &[T]) -> Vec<usize>
where
	for<'a> &'a [T]: PartialEq,
{
	let mut positions = Vec::new();
	loop {
		let skip = positions
			.last()
			.map(|&skip| skip + needle.len())
			.unwrap_or_default();
		if let Some(position) = haystack[skip..]
			.windows(needle.len())
			.position(|window| window == needle)
		{
			positions.push(skip + position);
		} else {
			break;
		}
	}
	positions
}
