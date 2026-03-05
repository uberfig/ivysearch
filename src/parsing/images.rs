use url::Url;

pub struct Image {
    pub url: Url,
    pub alt: Option<String>,
}

pub fn get_images() -> Vec<Image> {
    todo!()
}
