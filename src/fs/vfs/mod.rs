use alloc::vec::Vec;
use alloc::boxed::Box;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FS_FLAGS {
	File 		= 	0x01,
	Directory 	= 	0x02,
	CharDevice 	= 	0x03,
	BlockDevice = 	0x04,
	Pipe 		= 	0x05,
	Symlink 	= 	0x06,
	Mountpoint 	= 	0x08, // Is the file an active mountpoint?
}

pub struct FsNode {
	name: Vec<u8>, // The filename
	mask: u32, // The permissions mask
	uid: u32, // The owning user
	gid: u32, // The owning group
	flags: u32, // Includes the node type (Directory, file etc)
	inode: u32, // This is device-specific - provides a way for filesystems to identify files
	length: u32, // Size of the file in bytes
	impl_n: u32, // An implementation-defined number

	// read: ReadType, // Function pointers
	// write: WriteType,
	// open: OpenType,
	// close: CloseType,
	// read_dir: ReadDirType, // Returns the n'th child of a directory
 	// find_dir: FindDirType, // Try to find a child in a directory by name

	ptr: Box<FsNode>, // Used by mountpoints and symlinks
}

pub struct Dirent {
	name: Vec<u8>, // Filename
	ino: u32, // Inode number
}

pub trait FsFunctions {
	fn read(&self, offset: u32, size: u32, buffer: u8) -> u32;
	
	fn write(&self, offset: u32, size: u32, buffer: u8) -> u32;

	fn open(&self, read: u8, write: u8);

	fn close(&self);
}

pub fn read_fs(node: FsNode, offset: u32, size: u32, buffer: u8) {
	return node.read(offset, size, buffer);
}

pub fn open_fs(node: FsNode, offset: u32, size: u32, buffer: u8) {
	return node.open(offset, size, buffer);
}

pub fn close_fs(node: FsNode, offset: u32, size: u32, buffer: u8) {
	return node.close();
}

pub fn write_fs(node: FsNode, offset: u32, size: u32, buffer: u8) {
	return node.write(offset, size, buffer);
}

pub mod mount;