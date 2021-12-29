use crate::fs::vfs::FsFunctions;
use crate::serial_println;

pub struct RamDiskHeader {
	nfiles: u32, // The number of files in the ramdisk
}

pub struct RamDiskFileHeader {
    magic: u8,
    name: Vec<u8>,
    offset: u32,
    length: u32,
}


pub fn initialise_initrd(location: u32) -> FsNode {
	// Initialise the main and file header pointers and populate the root directory.
	let initrd_header = RamDiskHeader { nfiles: location };
	//let file_headers = Vec<RamDiskFileHeader> (initrd_file_header_t *) (location+sizeof(initrd_header_t));
	let file_headers = Vec<RamDiskFileHeader> = Vec::new();

	// Initialise the root directory.
	let initrd_root = (fs_node_t*)kmalloc(sizeof(fs_node_t));
	strcpy(initrd_root->name, "initrd");
	initrd_root->mask = initrd_root->uid = initrd_root->gid = initrd_root->inode = initrd_root->length = 0;
	initrd_root->flags = FS_DIRECTORY;
	initrd_root->read = 0;
	initrd_root->write = 0;
	initrd_root->open = 0;
	initrd_root->close = 0;
	initrd_root->readdir = &initrd_readdir;
	initrd_root->finddir = &initrd_finddir;
	initrd_root->ptr = 0;
	initrd_root->impl = 0;

	// Initialise the /dev directory (required!)
	initrd_dev = (fs_node_t*)kmalloc(sizeof(fs_node_t));
	strcpy(initrd_dev->name, "dev");
	initrd_dev->mask = initrd_dev->uid = initrd_dev->gid = initrd_dev->inode = initrd_dev->length = 0;
	initrd_dev->flags = FS_DIRECTORY;
	initrd_dev->read = 0;
	initrd_dev->write = 0;
	initrd_dev->open = 0;
	initrd_dev->close = 0;
	initrd_dev->readdir = &initrd_readdir;
	initrd_dev->finddir = &initrd_finddir;
	initrd_dev->ptr = 0;
	initrd_dev->impl = 0;

	root_nodes = (fs_node_t*)kmalloc(sizeof(fs_node_t) * initrd_header->nfiles);
	nroot_nodes = initrd_header->nfiles;

	// For every file...
	int i;
	for (i = 0; i < initrd_header->nfiles; i++)
	{
		// Edit the file's header - currently it holds the file offset
		// relative to the start of the ramdisk. We want it relative to the start
		// of memory.
		file_headers[i].offset += location;
		// Create a new file node.
		strcpy(root_nodes[i].name, &file_headers[i].name);
		root_nodes[i].mask = root_nodes[i].uid = root_nodes[i].gid = 0;
		root_nodes[i].length = file_headers[i].length;
		root_nodes[i].inode = i;
		root_nodes[i].flags = FS_FILE;
		root_nodes[i].read = &initrd_read;
		root_nodes[i].write = 0;
		root_nodes[i].readdir = 0;
		root_nodes[i].finddir = 0;
		root_nodes[i].open = 0;
		root_nodes[i].close = 0;
		root_nodes[i].impl = 0;
	}
	return initrd_root;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RamFS;

impl FsFunctions for RamFS {
	fn read(&self, offset: u32, size: u32, buffer: u8) -> u32 {
		serial_println!("offset: {}, size: {}, buf: {}", offset, size, buffer);

		return 10;
	}
	
	fn write(&self, offset: u32, size: u32, buffer: u8) -> u32 {
		return 11;
	}

	fn open(&self, read: u8, write: u8) {}

	fn close(&self) {}
}