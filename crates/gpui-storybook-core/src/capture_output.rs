use std::{io, path::Path};

pub(super) struct CaptureOutputStore;

impl CaptureOutputStore {
    pub(super) fn create_parent(path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    pub(super) fn save_png(
        image: &image::RgbaImage,
        path: impl AsRef<Path>,
    ) -> image::ImageResult<()> {
        image.save(path)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::CaptureOutputStore;

    static NEXT_PATH_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(name: &str) -> PathBuf {
        let id = NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gpui-storybook-capture-{name}-{}-{id}",
            std::process::id()
        ))
    }

    #[test]
    fn creates_nested_directories_and_writes_png_images() {
        let root = temporary_path("write");
        let path = root.join("nested").join("capture.png");
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 255]));

        CaptureOutputStore::create_parent(&path).unwrap();
        CaptureOutputStore::save_png(&image, &path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_directory_and_image_write_failures() {
        let root = temporary_path("errors");
        std::fs::write(&root, "not a directory").unwrap();
        let nested_path = root.join("capture.png");
        let image = image::RgbaImage::new(1, 1);

        assert!(CaptureOutputStore::create_parent(&nested_path).is_err());
        assert!(CaptureOutputStore::save_png(&image, &nested_path).is_err());
        std::fs::remove_file(root).unwrap();
    }
}
