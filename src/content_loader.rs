/// To receive HTTP raw content.
pub(crate) struct ContentLoader {
    buf: Vec<u8>,
    content_len: usize,
}

impl ContentLoader {
    pub(crate) fn new(content_len: usize) -> Self {
        ContentLoader { buf: Vec::new(), content_len }
    }

    /// Returns loaded buffer with 'content-len' and surplus.
    pub(crate) fn load_yet(&mut self, buf: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        self.buf.extend_from_slice(&buf);
        if self.buf.len() < self.content_len {
            return None;
        }

        // Loaded!

        let mut result_content = Vec::new();
        std::mem::swap(&mut self.buf, &mut result_content);

        let surplus = result_content[self.content_len..result_content.len()].to_vec();
        result_content.truncate(self.content_len);

        Some((result_content, surplus))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {
        let raw_data = b"Hello world!";
        let mut content_loader = ContentLoader::new(raw_data.len());
        if let Some((content, surplus)) = content_loader.load_yet(raw_data) {
            assert_eq!(content, raw_data);
            assert!(surplus.is_empty());
        } else {
            assert!(false);
        }

        let raw_data = b"Hello world!abc";
        let mut content_loader = ContentLoader::new(12);
        if let Some((content, surplus)) = content_loader.load_yet(raw_data) {
            assert_eq!(content, b"Hello world!");
            assert_eq!(surplus, b"abc");
        } else {
            assert!(false);
        }

        let raw_data = b"Hello ";
        let mut content_loader = ContentLoader::new(12);
        // need more data
        assert_eq!(content_loader.load_yet(raw_data), None);

        let raw_data = b"Hello world!";
        let mut content_loader = ContentLoader::new(raw_data.len());
        if let Some((content, surplus)) = content_loader.load_yet(raw_data) {
            assert_eq!(content, b"Hello world!");
            assert!(surplus.is_empty());
        } else {
            assert!(false);
        }
    }
}
