use martini::converter::Format;
use martini::converter::engine::archive::convert_archive;
use std::fs;
use std::io::Read;
use tempfile::tempdir;

#[test]
fn test_tar_to_zip_conversion() {
    let dir = tempdir().unwrap();
    let tar_file = dir.path().join("archive.tar");
    let zip_file = dir.path().join("archive.zip");

    // Create a simple tar
    let mut tar_builder = tar::Builder::new(fs::File::create(&tar_file).unwrap());
    let mut header = tar::Header::new_gnu();
    let data = b"hello martini";
    header.set_size(data.len() as u64);
    header.set_cksum();
    tar_builder
        .append_data(&mut header, "hello.txt", &data[..])
        .unwrap();
    tar_builder.finish().unwrap();

    let meta = convert_archive(&tar_file, &zip_file, Format::Tar, Format::Zip).unwrap();
    assert!(meta.size_bytes > 0);

    // Verify zip contains hello.txt
    let file = fs::File::open(&zip_file).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut entry = zip.by_name("hello.txt").expect("hello.txt must be in zip");
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    assert_eq!(content, "hello martini");
}

#[test]
fn test_zip_to_targz_conversion() {
    let dir = tempdir().unwrap();
    let zip_file = dir.path().join("source.zip");
    let targz_file = dir.path().join("dest.tar.gz");

    // Create a zip with a file
    {
        let file = fs::File::create(&zip_file).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("sample.txt", options).unwrap();
        use std::io::Write;
        zip.write_all(b"sample content").unwrap();
        zip.finish().unwrap();
    }

    let meta = convert_archive(&zip_file, &targz_file, Format::Zip, Format::Gz).unwrap();
    assert!(meta.size_bytes > 0);
    assert!(targz_file.exists());
}
